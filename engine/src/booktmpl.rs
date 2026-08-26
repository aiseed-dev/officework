//! **ブックの見た目 — テンプレート**(SEKKEI「エンジンの統一」4段目、
//! docs/sekkei/calc.ja.adoc「やる順」4)。
//!
//! `.adoc` のブックは**意味だけ**を持ちます(値と式とシート名)。
//! 列の幅・行の高さ・用紙の設定は見た目なので、隣の `テンプレート.adoc` が
//! 持ちます。writer と docx の関係、writer とテンプレートの関係と同じです。
//!
//! *中身は表です*(SEKKEI「スタイルの定義は表で書く」)。設定の書き方
//! (`キー = 値`)ではなく、穴の空いた文書と同じ**表**にしてあります。
//!
//! ....
//! .用紙
//! |===
//! |シート |大きさ |向き |余白 |目盛線
//!
//! |売上台帳 |A4 |横 |20 |true
//! |===
//!
//! .列幅
//! |===
//! |シート |列 |幅
//!
//! |売上台帳 |A |20
//! |===
//! ....
//!
//! *表の読み書きは書き直しません。* `kumihan::adoc` に渡します。
//!
//! *テンプレートの持ち主は、指示する人です*(2026-08-18 発注者)。
//! だから**配られたテンプレートは書き替えません** — 呼ぶ側は、既にある
//! ファイルを上書きしないでください。

use crate::book::{Book, FreezePane, Pos, ProtectAllow, Sheet};
use crate::{Block, Cellbox, Document, Table};

/// 1枚ぶんの見た目。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SheetLook {
    pub name: String,
    /// (列, 幅)。列は 0 から
    pub col_width: Vec<(u32, f32)>,
    /// (行, 高さ)。行は 0 から
    pub row_height: Vec<(u32, f32)>,
    /// 用紙の大きさ(xlsx の番号。9 = A4)
    pub paper_size: Option<u32>,
    pub landscape: Option<bool>,
    /// 余白(mm。左, 右, 上, 下)
    pub margins_mm: Option<(f32, f32, f32, f32)>,
    pub print_gridlines: Option<bool>,
    pub zoom_scale: Option<u32>,
    /// 印刷の倍率(%)
    pub print_scale: Option<u32>,
    /// 横に何枚で収めるか。0 は「指定なし」
    pub fit_to_w: Option<u32>,
    /// 縦に何枚で収めるか。0 は「指定なし」
    pub fit_to_h: Option<u32>,
    /// 行番号と列番号を刷るか
    pub print_headings: Option<bool>,
    /// 各ページの頭で繰り返す行(0 から。始めと終わり)
    pub print_title_rows: Option<(u32, u32)>,
    /// 各ページの左で繰り返す列(0 から。始めと終わり)
    pub print_title_cols: Option<(u32, u32)>,
    /// 手で入れた横の改ページ(0 からの行)
    pub row_breaks: Vec<u32>,
    /// 手で入れた縦の改ページ(0 からの列)
    pub col_breaks: Vec<u32>,
    pub header: Option<String>,
    pub footer: Option<String>,
    pub header_even: Option<String>,
    pub footer_even: Option<String>,
    pub header_first: Option<String>,
    pub footer_first: Option<String>,
    /// 奇数頁と偶数頁で分けるか
    pub hf_diff_odd_even: Option<bool>,
    /// 先頭の頁を分けるか
    pub hf_diff_first: Option<bool>,
    /// 固定枠(固定する行の数, 列の数)
    pub freeze: Option<(u32, u32)>,
    pub show_gridlines: Option<bool>,
    pub show_formulas: Option<bool>,
    /// 右から左へ書くか
    pub rtl: Option<bool>,
    /// このシートを隠すか
    pub hidden: Option<bool>,
    /// シート見出しの色(RRGGBB)
    pub tab_color: Option<String>,
    /// シートを保護するか
    pub protected: Option<bool>,
    /// 保護中も許す操作の名前
    pub protect_allow: Option<Vec<String>>,
    /// (行, 段, 畳んでいるか)
    pub row_outline: Vec<(u32, u8, bool)>,
    /// (列, 段, 畳んでいるか)
    pub col_outline: Vec<(u32, u8, bool)>,
    /// 何も指定していない列の幅
    pub default_col_width: Option<f32>,
    /// 何も指定していない行の高さ
    pub default_row_height: Option<f32>,
}

/// ブックの見た目ぜんぶ。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookTheme {
    pub sheets: Vec<SheetLook>,
}

impl BookTheme {
    /// 持っている物が何も無いか(何も無ければテンプレートを書く意味がない)
    pub fn is_empty(&self) -> bool {
        self.sheets.iter().all(|s| {
            s.col_width.is_empty()
                && s.row_height.is_empty()
                && s.paper_size.is_none()
                && s.landscape.is_none()
                && s.margins_mm.is_none()
                && s.print_gridlines.is_none()
                && s.zoom_scale.is_none()
                && s.print_scale.is_none()
                && s.fit_to_w.is_none()
                && s.fit_to_h.is_none()
                && s.print_headings.is_none()
                && s.print_title_rows.is_none()
                && s.print_title_cols.is_none()
                && s.row_breaks.is_empty()
                && s.col_breaks.is_empty()
                && s.header.is_none()
                && s.footer.is_none()
                && s.header_even.is_none()
                && s.footer_even.is_none()
                && s.header_first.is_none()
                && s.footer_first.is_none()
                && s.hf_diff_odd_even.is_none()
                && s.hf_diff_first.is_none()
                && s.freeze.is_none()
                && s.show_gridlines.is_none()
                && s.show_formulas.is_none()
                && s.rtl.is_none()
                && s.hidden.is_none()
                && s.tab_color.is_none()
                && s.protected.is_none()
                && s.protect_allow.is_none()
                && s.row_outline.is_empty()
                && s.col_outline.is_empty()
                && s.default_col_width.is_none()
                && s.default_row_height.is_none()
        })
    }

    fn sheet(&mut self, name: &str) -> &mut SheetLook {
        if let Some(i) = self.sheets.iter().position(|s| s.name == name) {
            return &mut self.sheets[i];
        }
        self.sheets.push(SheetLook { name: name.to_string(), ..Default::default() });
        self.sheets.last_mut().expect("いま入れた")
    }
}

/// ブックから見た目を取り出す。
pub fn from_book(b: &Book) -> BookTheme {
    let mut t = BookTheme::default();
    for s in &b.sheets {
        let look = SheetLook {
            name: s.name.clone(),
            col_width: s.col_width.iter().map(|(k, v)| (*k, *v)).collect(),
            row_height: s.row_height.iter().map(|(k, v)| (*k, *v)).collect(),
            paper_size: s.paper_size,
            landscape: s.landscape.then_some(true),
            margins_mm: s.margins_mm,
            print_gridlines: s.print_gridlines.then_some(true),
            zoom_scale: s.zoom_scale,
            print_scale: s.print_scale,
            fit_to_w: s.fit_to_w,
            fit_to_h: s.fit_to_h,
            print_headings: s.print_headings.then_some(true),
            print_title_rows: s.print_title_rows,
            print_title_cols: s.print_title_cols,
            row_breaks: s.row_breaks.clone(),
            col_breaks: s.col_breaks.clone(),
            header: s.header.clone(),
            footer: s.footer.clone(),
            header_even: s.header_even.clone(),
            footer_even: s.footer_even.clone(),
            header_first: s.header_first.clone(),
            footer_first: s.footer_first.clone(),
            hf_diff_odd_even: s.hf_diff_odd_even.then_some(true),
            hf_diff_first: s.hf_diff_first.then_some(true),
            freeze: s.freeze.as_ref().map(|f| (f.frozen_rows, f.frozen_columns)),
            show_gridlines: s.show_gridlines,
            show_formulas: s.show_formulas,
            rtl: s.rtl.then_some(true),
            hidden: s.hidden.then_some(true),
            tab_color: s.tab_color.clone(),
            protected: s.protected.then_some(true),
            protect_allow: s.protected.then(|| allow_names(&s.protect_allow)),
            // **畳んだ印は段の指定と別**です。畳むボタンの載る行には段が
            // 無いことがあるので、両方を合わせて拾います(2026-08-26)
            row_outline: outline_rows(&s.row_outline, &s.row_collapsed),
            col_outline: outline_rows(&s.col_outline, &s.col_collapsed),
            default_col_width: s.default_col_width,
            default_row_height: s.default_row_height,
        };
        t.sheets.push(look);
    }
    t
}

/// 見た目をブックに当てる。**そのシートが無ければ黙って飛ばします**
/// (テンプレートは別のブックにも使えるので、名前が合わないのは普通のこと)。
pub fn apply(t: &BookTheme, b: &mut Book) {
    for look in &t.sheets {
        let Some(s) = b.sheets.iter_mut().find(|s| s.name == look.name) else { continue };
        for (c, w) in &look.col_width {
            s.col_width.insert(*c, *w);
        }
        for (r, h) in &look.row_height {
            s.row_height.insert(*r, *h);
        }
        if let Some(p) = look.paper_size {
            s.paper_size = Some(p);
        }
        if let Some(l) = look.landscape {
            s.landscape = l;
        }
        if let Some(m) = look.margins_mm {
            s.margins_mm = Some(m);
        }
        if let Some(g) = look.print_gridlines {
            s.print_gridlines = g;
        }
        if let Some(z) = look.zoom_scale {
            s.zoom_scale = Some(z);
        }
        if let Some(v) = look.print_scale {
            s.print_scale = Some(v);
        }
        if let Some(v) = look.fit_to_w {
            s.fit_to_w = Some(v);
        }
        if let Some(v) = look.fit_to_h {
            s.fit_to_h = Some(v);
        }
        if let Some(v) = look.print_headings {
            s.print_headings = v;
        }
        if let Some(v) = look.print_title_rows {
            s.print_title_rows = Some(v);
        }
        if let Some(v) = look.print_title_cols {
            s.print_title_cols = Some(v);
        }
        if !look.row_breaks.is_empty() {
            s.row_breaks = look.row_breaks.clone();
        }
        if !look.col_breaks.is_empty() {
            s.col_breaks = look.col_breaks.clone();
        }
        // ヘッダーとフッター。**空の字も指定のうち**なので、Some なら入れます
        for (mine, theirs) in [
            (&look.header, &mut s.header),
            (&look.footer, &mut s.footer),
            (&look.header_even, &mut s.header_even),
            (&look.footer_even, &mut s.footer_even),
            (&look.header_first, &mut s.header_first),
            (&look.footer_first, &mut s.footer_first),
        ] {
            if let Some(v) = mine {
                *theirs = Some(v.clone());
            }
        }
        if let Some(v) = look.hf_diff_odd_even {
            s.hf_diff_odd_even = v;
        }
        if let Some(v) = look.hf_diff_first {
            s.hf_diff_first = v;
        }
        if let Some((r, c)) = look.freeze {
            s.freeze = Some(FreezePane { frozen_rows: r, frozen_columns: c });
        }
        if look.show_gridlines.is_some() {
            s.show_gridlines = look.show_gridlines;
        }
        if look.show_formulas.is_some() {
            s.show_formulas = look.show_formulas;
        }
        if let Some(v) = look.rtl {
            s.rtl = v;
        }
        if let Some(v) = look.hidden {
            s.hidden = v;
        }
        if look.tab_color.is_some() {
            s.tab_color = look.tab_color.clone();
        }
        if let Some(v) = look.protected {
            s.protected = v;
        }
        if let Some(names) = &look.protect_allow {
            s.protect_allow = allow_from(names);
        }
        // 段が 0 は「段の指定なし」— 畳んだ印だけの行です
        for (r, lv, folded) in &look.row_outline {
            if *lv > 0 {
                s.row_outline.insert(*r, *lv);
            }
            if *folded {
                s.row_collapsed.insert(*r);
            }
        }
        for (c, lv, folded) in &look.col_outline {
            if *lv > 0 {
                s.col_outline.insert(*c, *lv);
            }
            if *folded {
                s.col_collapsed.insert(*c);
            }
        }
        if let Some(v) = look.default_col_width {
            s.default_col_width = Some(v);
        }
        if let Some(v) = look.default_row_height {
            s.default_row_height = Some(v);
        }
    }
}

// ---------- 書く ----------

/// テンプレートの字にする。
pub fn write(t: &BookTheme) -> String {
    let mut d = Document::default();
    if let Some(tb) = paper_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = width_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = height_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = print_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = break_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = hf_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = view_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = outline_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = protect_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    crate::adoc::write(&d)
}

fn cell(s: &str) -> Cellbox {
    Cellbox { paragraphs: Document::plain(s).paragraphs().cloned().collect(), ..Default::default() }
}

fn table(title: &str, heading: &[&str], rows: Vec<Vec<String>>) -> Option<Table> {
    if rows.is_empty() {
        return None;
    }
    let mut t = Table {
        title: Some(title.to_string()),
        header_row: true,
        rows: vec![heading.iter().map(|h| cell(h)).collect()],
        ..Default::default()
    };
    for r in rows {
        t.rows.push(r.iter().map(|x| cell(x)).collect());
    }
    Some(t)
}



/// **保護中も許す操作の名前。** 表と `ProtectAllow` の欄を1対1で結びます。
///
/// 名前は Excel の「シートの保護」の小窓の言い方に寄せてあります。
/// **欄を足したらここにも足すこと** — `every_protect_flag_has_a_name` が
/// 数を確かめます。
pub const ALLOW_NAMES: &[(&str, fn(&mut ProtectAllow))] = &[
    ("ロックされたセルの選択", |a| a.select_locked = true),
    ("ロックされていないセルの選択", |a| a.select_unlocked = true),
    ("セルの書式設定", |a| a.format_cells = true),
    ("列の書式設定", |a| a.format_cols = true),
    ("行の書式設定", |a| a.format_rows = true),
    ("列の挿入", |a| a.insert_cols = true),
    ("行の挿入", |a| a.insert_rows = true),
    ("ハイパーリンクの挿入", |a| a.insert_links = true),
    ("列の削除", |a| a.delete_cols = true),
    ("行の削除", |a| a.delete_rows = true),
    ("並べ替え", |a| a.sort = true),
    ("オートフィルターの使用", |a| a.autofilter = true),
    ("ピボットテーブルの使用", |a| a.pivot = true),
    ("オブジェクトの編集", |a| a.objects = true),
];

/// 段の指定と畳んだ印を合わせて (位置, 段, 畳むか) の並びにする。
///
/// **どちらか片方しか無い所も落としません。** 畳むボタンの載る行には
/// 段の指定が無いことがあり、段だけ書くとその行の畳みが消えます。
fn outline_rows(
    levels: &std::collections::BTreeMap<u32, u8>,
    folded: &std::collections::BTreeSet<u32>,
) -> Vec<(u32, u8, bool)> {
    let mut at: Vec<u32> = levels.keys().copied().chain(folded.iter().copied()).collect();
    at.sort_unstable();
    at.dedup();
    at.iter()
        .map(|k| (*k, levels.get(k).copied().unwrap_or(0), folded.contains(k)))
        .collect()
}

/// いま許している操作の名前を並べる
fn allow_names(a: &ProtectAllow) -> Vec<String> {
    let on = [
        a.select_locked, a.select_unlocked, a.format_cells, a.format_cols, a.format_rows,
        a.insert_cols, a.insert_rows, a.insert_links, a.delete_cols, a.delete_rows,
        a.sort, a.autofilter, a.pivot, a.objects,
    ];
    ALLOW_NAMES
        .iter()
        .zip(on)
        .filter(|(_, yes)| *yes)
        .map(|((n, _), _)| n.to_string())
        .collect()
}

/// 名前の並びから許可を組み立てる。**知らない名前は黙って飛ばします**
fn allow_from(names: &[String]) -> ProtectAllow {
    // **全部切った所から組み立てます。** 既定の `ProtectAllow` は
    // 「ロックされたセルの選択」が入なので、`Default` から始めると
    // 表に書いていない許可が勝手に付きます
    let mut a = ProtectAllow {
        select_locked: false, select_unlocked: false, format_cells: false,
        format_cols: false, format_rows: false, insert_cols: false, insert_rows: false,
        insert_links: false, delete_cols: false, delete_rows: false, sort: false,
        autofilter: false, pivot: false, objects: false,
    };
    for n in names {
        if let Some((_, set)) = ALLOW_NAMES.iter().find(|(name, _)| name == n) {
            set(&mut a);
        }
    }
    a
}

/// 画面の設定。**シートを開いたときの見え方**です。
fn view_table(t: &BookTheme) -> Option<Table> {
    let rows: Vec<Vec<String>> = t
        .sheets
        .iter()
        .filter(|s| {
            s.freeze.is_some()
                || s.show_gridlines.is_some()
                || s.show_formulas.is_some()
                || s.rtl.is_some()
                || s.hidden.is_some()
                || s.tab_color.is_some()
                || s.default_col_width.is_some()
                || s.default_row_height.is_some()
        })
        .map(|s| {
            vec![
                s.name.clone(),
                s.freeze.map(|(r, c)| format!("{r},{c}")).unwrap_or_default(),
                yes_no(s.show_gridlines),
                yes_no(s.show_formulas),
                yes_no(s.rtl),
                yes_no(s.hidden),
                s.tab_color.clone().unwrap_or_default(),
                s.default_col_width.map(numbers).unwrap_or_default(),
                s.default_row_height.map(numbers).unwrap_or_default(),
            ]
        })
        .collect();
    table(
        "画面",
        &["シート", "固定", "目盛線", "数式", "右横書き", "隠す", "見出しの色", "既定の列幅", "既定の行の高さ"],
        rows,
    )
}

fn read_view(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        if name.is_empty() {
            continue;
        }
        let s = t.sheet(name);
        if let Some((r, c)) = pick(row, 1).split_once(',') {
            if let (Ok(r), Ok(c)) = (r.trim().parse(), c.trim().parse()) {
                s.freeze = Some((r, c));
            }
        }
        s.show_gridlines = read_yes_no(pick(row, 2)).or(s.show_gridlines);
        s.show_formulas = read_yes_no(pick(row, 3)).or(s.show_formulas);
        s.rtl = read_yes_no(pick(row, 4)).or(s.rtl);
        s.hidden = read_yes_no(pick(row, 5)).or(s.hidden);
        let color = pick(row, 6);
        if !color.is_empty() {
            s.tab_color = Some(color.to_string());
        }
        if let Ok(v) = pick(row, 7).parse() {
            s.default_col_width = Some(v);
        }
        if let Ok(v) = pick(row, 8).parse() {
            s.default_row_height = Some(v);
        }
    }
}

/// グループ化(アウトライン)。**1つの段に1行**です。
fn outline_table(t: &BookTheme) -> Option<Table> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for s in &t.sheets {
        for (r, lv, folded) in &s.row_outline {
            rows.push(vec![
                s.name.clone(), "行".into(), (r + 1).to_string(),
                lv.to_string(), yes_no(Some(*folded)),
            ]);
        }
        for (c, lv, folded) in &s.col_outline {
            rows.push(vec![
                s.name.clone(), "列".into(), col_name(*c),
                lv.to_string(), yes_no(Some(*folded)),
            ]);
        }
    }
    table("グループ化", &["シート", "種類", "位置", "段", "畳む"], rows)
}

fn read_outline(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        if name.is_empty() {
            continue;
        }
        let kind = pick(row, 1).to_string();
        let at = pick(row, 2).to_string();
        let Ok(level) = pick(row, 3).parse::<u8>() else { continue };
        let folded = read_yes_no(pick(row, 4)).unwrap_or(false);
        let s = t.sheet(name);
        match kind.as_str() {
            "行" => {
                if let Ok(n) = at.parse::<u32>() {
                    if n > 0 {
                        s.row_outline.push((n - 1, level, folded));
                    }
                }
            }
            "列" => {
                if let Some(c) = col_index(&at) {
                    s.col_outline.push((c, level, folded));
                }
            }
            _ => {}
        }
    }
}

/// シートの保護。**許す操作は名前を並べます**(数の並びにしない —
/// テンプレートは人が読んで直す物です)。
fn protect_table(t: &BookTheme) -> Option<Table> {
    let rows: Vec<Vec<String>> = t
        .sheets
        .iter()
        .filter(|s| s.protected.is_some())
        .map(|s| {
            vec![
                s.name.clone(),
                yes_no(s.protected),
                s.protect_allow.clone().unwrap_or_default().join("、"),
            ]
        })
        .collect();
    table("保護", &["シート", "保護", "許す操作"], rows)
}

fn read_protect(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        if name.is_empty() {
            continue;
        }
        let on = read_yes_no(pick(row, 1));
        let names: Vec<String> = pick(row, 2)
            .split(['、', ','])
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        let s = t.sheet(name);
        s.protected = on.or(s.protected);
        if s.protected == Some(true) {
            s.protect_allow = Some(names);
        }
    }
}

/// 印刷の設定。**用紙の表と分けたのは、列が多くなりすぎるから**です。
fn print_table(t: &BookTheme) -> Option<Table> {
    let rows: Vec<Vec<String>> = t
        .sheets
        .iter()
        .filter(|s| {
            s.print_scale.is_some()
                || s.fit_to_w.is_some()
                || s.fit_to_h.is_some()
                || s.print_headings.is_some()
                || s.print_title_rows.is_some()
                || s.print_title_cols.is_some()
        })
        .map(|s| {
            vec![
                s.name.clone(),
                s.print_scale.map(|v| v.to_string()).unwrap_or_default(),
                s.fit_to_w.map(|v| v.to_string()).unwrap_or_default(),
                s.fit_to_h.map(|v| v.to_string()).unwrap_or_default(),
                yes_no(s.print_headings),
                s.print_title_rows.map(|(a, b)| format!("{}:{}", a + 1, b + 1)).unwrap_or_default(),
                s.print_title_cols.map(|(a, b)| format!("{}:{}", col_name(a), col_name(b))).unwrap_or_default(),
            ]
        })
        .collect();
    table(
        "印刷",
        &["シート", "倍率", "横に収める", "縦に収める", "行列番号", "タイトル行", "タイトル列"],
        rows,
    )
}

fn read_print(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        if name.is_empty() {
            continue;
        }
        let s = t.sheet(name);
        if let Ok(v) = pick(row, 1).parse() {
            s.print_scale = Some(v);
        }
        if let Ok(v) = pick(row, 2).parse() {
            s.fit_to_w = Some(v);
        }
        if let Ok(v) = pick(row, 3).parse() {
            s.fit_to_h = Some(v);
        }
        s.print_headings = read_yes_no(pick(row, 4)).or(s.print_headings);
        s.print_title_rows = read_rows(pick(row, 5)).or(s.print_title_rows);
        s.print_title_cols = read_cols(pick(row, 6)).or(s.print_title_cols);
    }
}

/// 手で入れた改ページ。**行は番号、列は綴りの名前**で書きます
/// (画面の見出しと同じ言い方にするため)。
fn break_table(t: &BookTheme) -> Option<Table> {
    let rows: Vec<Vec<String>> = t
        .sheets
        .iter()
        .filter(|s| !s.row_breaks.is_empty() || !s.col_breaks.is_empty())
        .map(|s| {
            vec![
                s.name.clone(),
                s.row_breaks.iter().map(|r| (r + 1).to_string()).collect::<Vec<_>>().join(","),
                s.col_breaks.iter().map(|c| col_name(*c)).collect::<Vec<_>>().join(","),
            ]
        })
        .collect();
    table("改ページ", &["シート", "行", "列"], rows)
}

fn read_break(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        if name.is_empty() {
            continue;
        }
        let s = t.sheet(name);
        let rb: Vec<u32> = pick(row, 1)
            .split(',')
            .filter_map(|x| x.trim().parse::<u32>().ok())
            .filter(|n| *n > 0)
            .map(|n| n - 1)
            .collect();
        if !rb.is_empty() {
            s.row_breaks = rb;
        }
        let cb: Vec<u32> = pick(row, 2).split(',').filter_map(|x| col_index(x.trim())).collect();
        if !cb.is_empty() {
            s.col_breaks = cb;
        }
    }
}

/// ヘッダーとフッター。**1つの位置に1行**です。
///
/// 「奇数と偶数を分ける」「先頭の頁を分ける」の入切は、その位置の行が
/// あるかどうかで表します。**字が空でも行があれば入**です — 分ける指定を
/// して中身を書いていない状態は、本家にもあるためです。
fn hf_table(t: &BookTheme) -> Option<Table> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for s in &t.sheets {
        for (label, v) in [
            ("ヘッダー", &s.header),
            ("フッター", &s.footer),
        ] {
            if let Some(x) = v {
                rows.push(vec![s.name.clone(), label.into(), x.clone()]);
            }
        }
        if s.hf_diff_odd_even == Some(true) {
            for (label, v) in [("偶数ヘッダー", &s.header_even), ("偶数フッター", &s.footer_even)] {
                rows.push(vec![s.name.clone(), label.into(), v.clone().unwrap_or_default()]);
            }
        }
        if s.hf_diff_first == Some(true) {
            for (label, v) in [("先頭ヘッダー", &s.header_first), ("先頭フッター", &s.footer_first)] {
                rows.push(vec![s.name.clone(), label.into(), v.clone().unwrap_or_default()]);
            }
        }
    }
    table("ヘッダーとフッター", &["シート", "位置", "文字"], rows)
}

fn read_hf(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        if name.is_empty() {
            continue;
        }
        let where_at = pick(row, 1).to_string();
        let text = pick(row, 2).to_string();
        let s = t.sheet(name);
        match where_at.as_str() {
            "ヘッダー" => s.header = Some(text),
            "フッター" => s.footer = Some(text),
            "偶数ヘッダー" => {
                s.header_even = Some(text);
                s.hf_diff_odd_even = Some(true);
            }
            "偶数フッター" => {
                s.footer_even = Some(text);
                s.hf_diff_odd_even = Some(true);
            }
            "先頭ヘッダー" => {
                s.header_first = Some(text);
                s.hf_diff_first = Some(true);
            }
            "先頭フッター" => {
                s.footer_first = Some(text);
                s.hf_diff_first = Some(true);
            }
            _ => {}
        }
    }
}

/// 入切を字に。指定なしは空
fn yes_no(v: Option<bool>) -> String {
    match v {
        Some(true) => "true".into(),
        Some(false) => "false".into(),
        None => String::new(),
    }
}

fn read_yes_no(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// `1:1` を (0, 0) に。空なら None
fn read_rows(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once(':')?;
    let a: u32 = a.trim().parse().ok()?;
    let b: u32 = b.trim().parse().ok()?;
    (a > 0 && b > 0).then(|| (a - 1, b - 1))
}

/// `A:A` を (0, 0) に。空なら None
fn read_cols(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once(':')?;
    Some((col_index(a.trim())?, col_index(b.trim())?))
}

/// 数を字にする(整数はそのまま、小数は要るぶんだけ)
fn numbers(v: f32) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

fn paper_table(t: &BookTheme) -> Option<Table> {
    let rows: Vec<Vec<String>> = t
        .sheets
        .iter()
        .filter(|s| {
            s.paper_size.is_some()
                || s.landscape.is_some()
                || s.margins_mm.is_some()
                || s.print_gridlines.is_some()
                || s.zoom_scale.is_some()
        })
        .map(|s| {
            vec![
                s.name.clone(),
                s.paper_size.map(paper_name).unwrap_or_default(),
                match s.landscape {
                    Some(true) => "横".into(),
                    Some(false) => "縦".into(),
                    None => String::new(),
                },
                s.margins_mm.map(|(l, r, tp, b)| format!("{},{},{},{}", numbers(l), numbers(r), numbers(tp), numbers(b))).unwrap_or_default(),
                match s.print_gridlines {
                    Some(true) => "true".into(),
                    Some(false) => "false".into(),
                    None => String::new(),
                },
                s.zoom_scale.map(|z| z.to_string()).unwrap_or_default(),
            ]
        })
        .collect();
    table("用紙", &["シート", "大きさ", "向き", "余白", "目盛線", "拡大"], rows)
}

fn width_table(t: &BookTheme) -> Option<Table> {
    let mut rows = Vec::new();
    for s in &t.sheets {
        for (c, w) in &s.col_width {
            rows.push(vec![s.name.clone(), col_name(*c), numbers(*w)]);
        }
    }
    table("列幅", &["シート", "列", "幅"], rows)
}

fn height_table(t: &BookTheme) -> Option<Table> {
    let mut rows = Vec::new();
    for s in &t.sheets {
        for (r, h) in &s.row_height {
            rows.push(vec![s.name.clone(), (r + 1).to_string(), numbers(*h)]);
        }
    }
    table("行の高さ", &["シート", "行", "高さ"], rows)
}

/// 列の番号を A1 の綴りの列の名にする(0 → A)
fn col_name(c: u32) -> String {
    let a1 = Pos::new(0, c).a1();
    a1.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string()
}

/// 用紙の番号を名前に(xlsx の番号は Excel の決め)
fn paper_name(n: u32) -> String {
    match n {
        8 => "A3".into(),
        9 => "A4".into(),
        11 => "A5".into(),
        12 => "B4".into(),
        13 => "B5".into(),
        1 => "Letter".into(),
        5 => "Legal".into(),
        // 知らない番号は**番号のまま返す**(黙って A4 にしない)
        other => other.to_string(),
    }
}

fn paper_no(s: &str) -> Option<u32> {
    match s.trim().to_ascii_uppercase().as_str() {
        "A3" => Some(8),
        "A4" => Some(9),
        "A5" => Some(11),
        "B4" => Some(12),
        "B5" => Some(13),
        "LETTER" => Some(1),
        "LEGAL" => Some(5),
        other => other.parse().ok(),
    }
}

// ---------- 読む ----------

/// テンプレートの字を読む。知らない表は**黙って飛ばします**
/// (テンプレートには writer 向けの節も混じるため)。
pub fn parse(src: &str) -> Result<BookTheme, String> {
    let doc = crate::adoc::parse(src)?;
    let mut t = BookTheme::default();
    for b in &doc.blocks {
        let Block::Table(tb) = b else { continue };
        let Some(title) = tb.title.as_deref() else { continue };
        let rows = tb.text_rows();
        // 1行目は見出し
        let body = if tb.header_row && !rows.is_empty() { &rows[1..] } else { &rows[..] };
        match title {
            "用紙" => read_paper(&mut t, body),
            "列幅" => read_width(&mut t, body),
            "行の高さ" => read_height(&mut t, body),
            "印刷" => read_print(&mut t, body),
            "改ページ" => read_break(&mut t, body),
            "ヘッダーとフッター" => read_hf(&mut t, body),
            "画面" => read_view(&mut t, body),
            "グループ化" => read_outline(&mut t, body),
            "保護" => read_protect(&mut t, body),
            _ => {}
        }
    }
    Ok(t)
}

fn pick(row: &[String], i: usize) -> &str {
    row.get(i).map(|s| s.trim()).unwrap_or("")
}

fn read_paper(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        if name.is_empty() {
            continue;
        }
        let s = t.sheet(name);
        let size = pick(row, 1);
        if !size.is_empty() {
            s.paper_size = paper_no(size);
        }
        match pick(row, 2) {
            "横" => s.landscape = Some(true),
            "縦" => s.landscape = Some(false),
            _ => {}
        }
        let margins: Vec<f32> = pick(row, 3).split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if margins.len() == 4 {
            s.margins_mm = Some((margins[0], margins[1], margins[2], margins[3]));
        } else if margins.len() == 1 {
            // 1つだけなら四方とも同じ
            s.margins_mm = Some((margins[0], margins[0], margins[0], margins[0]));
        }
        match pick(row, 4).to_ascii_lowercase().as_str() {
            "true" => s.print_gridlines = Some(true),
            "false" => s.print_gridlines = Some(false),
            _ => {}
        }
        if let Ok(z) = pick(row, 5).parse() {
            s.zoom_scale = Some(z);
        }
    }
}

fn read_width(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        let Some(c) = col_index(pick(row, 1)) else { continue };
        let Ok(w) = pick(row, 2).parse::<f32>() else { continue };
        if !name.is_empty() {
            t.sheet(name).col_width.push((c, w));
        }
    }
}

fn read_height(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        let Ok(r) = pick(row, 1).parse::<u32>() else { continue };
        let Ok(h) = pick(row, 2).parse::<f32>() else { continue };
        if !name.is_empty() && r >= 1 {
            t.sheet(name).row_height.push((r - 1, h));
        }
    }
}

/// 列の名(`A`)を番号に。`Pos::parse` に1行目を足して解かせます
fn col_index(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Pos::parse(&format!("{s}1")).map(|p| p.col)
}

/// このブックのフォルダのテンプレートを探す。
///
/// 名前の決めは SEKKEI「ファイルの名前 — 二重の拡張子で種類を言う」
/// (2026-08-18 発注者)。*見た目の元は `名前.tmpl.adoc`* です。
///
/// **フォルダの既定は `.tmpl.adoc` が1枚ならそれ。** 何枚もあるときは
/// どれを使うか決められないので `None` を返します(黙って1枚目を選ばない —
/// 書き出し先ごとに `web.tmpl.adoc` `print.tmpl.adoc` と分ける使い方が
/// あるので、選ぶのは人の仕事です)。
pub fn find_for(book: &std::path::Path) -> Option<std::path::PathBuf> {
    let dir = book.parent().unwrap_or(std::path::Path::new("."));
    let mut cands: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".tmpl.adoc")))
        .collect();
    cands.sort();
    (cands.len() == 1).then(|| cands.remove(0))
}

/// このブックのフォルダに**新しく置く**テンプレートの径路。
/// 既定の名前は `既定.tmpl.adoc` です。
pub fn default_path(book: &std::path::Path) -> std::path::PathBuf {
    book.parent().unwrap_or(std::path::Path::new(".")).join("既定.tmpl.adoc")
}

/// 見た目を落とさずに済むよう、`Sheet` から見た目だけを消す。
/// `.adoc` に書くときに使います(意味だけを書くため)。
pub fn strip(s: &mut Sheet) {
    s.col_width.clear();
    s.row_height.clear();
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::book::Cell;

    fn ledger() -> Book {
        let mut b = Book::new();
        b.sheets[0].name = "売上台帳".into();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("月"));
        b.sheets[0].col_width.insert(0, 20.0);
        b.sheets[0].col_width.insert(3, 12.5);
        b.sheets[0].row_height.insert(0, 24.0);
        b.sheets[0].paper_size = Some(9);
        b.sheets[0].landscape = true;
        b.sheets[0].margins_mm = Some((20.0, 20.0, 15.0, 15.0));
        b
    }

    #[test]
    fn the_written_text_is_a_table() {
        let src = write(&from_book(&ledger()));
        assert!(src.contains(".用紙"), "用紙の表が無い:\n{src}");
        assert!(src.contains(".列幅"), "列幅の表が無い:\n{src}");
        assert!(src.contains("|売上台帳 |A |20"), "列幅の行が無い:\n{src}");
        assert!(src.contains("A4"), "用紙の名前が番号のまま:\n{src}");
    }

    #[test]
    fn the_look_survives_a_round_trip() {
        let from = from_book(&ledger());
        let back = parse(&write(&from)).expect("読めない");
        assert_eq!(back, from, "往復で見た目が変わった");
    }

    /// **当てるとブックに戻る。** 意味だけの `.adoc` と組み合わせる形
    #[test]
    fn applies_to_a_book() {
        let t = from_book(&ledger());
        let mut b = Book::new();
        b.sheets[0].name = "売上台帳".into();
        apply(&t, &mut b);
        assert_eq!(b.sheets[0].col_width.get(&0), Some(&20.0));
        assert_eq!(b.sheets[0].col_width.get(&3), Some(&12.5));
        assert_eq!(b.sheets[0].row_height.get(&0), Some(&24.0));
        assert_eq!(b.sheets[0].paper_size, Some(9));
        assert!(b.sheets[0].landscape);
        assert_eq!(b.sheets[0].margins_mm, Some((20.0, 20.0, 15.0, 15.0)));
    }

    /// 名前の合わないシートは**黙って飛ばす**(テンプレートは使い回せる)
    #[test]
    fn unknown_sheets_are_skipped() {
        let t = from_book(&ledger());
        let mut b = Book::new();
        b.sheets[0].name = "別の名前".into();
        apply(&t, &mut b);
        assert!(b.sheets[0].col_width.is_empty(), "知らないシートに当ててしまった");
    }

    /// 知らない表は飛ばす(writer 向けの節が混じっていても落ちない)
    #[test]
    fn unknown_tables_are_skipped() {
        let t = parse(".スタイル\n|===\n|名前 |大きさ\n\n|見出し1 |16\n|===\n").expect("読めない");
        assert!(t.is_empty());
    }

    /// 余白は1つだけ書けば四方とも同じ
    #[test]
    fn one_margin_value_is_enough() {
        let t = parse(".用紙\n|===\n|シート |大きさ |向き |余白\n\n|表 |A4 |縦 |20\n|===\n").expect("読めない");
        assert_eq!(t.sheets[0].margins_mm, Some((20.0, 20.0, 20.0, 20.0)));
        assert_eq!(t.sheets[0].paper_size, Some(9));
        assert_eq!(t.sheets[0].landscape, Some(false));
    }

    /// 知らない用紙の番号は**番号のまま**(黙って A4 にしない)
    #[test]
    fn unknown_paper_keeps_its_number() {
        assert_eq!(paper_name(99), "99");
        assert_eq!(paper_no("99"), Some(99));
    }
}

/// **許す操作の名前が `ProtectAllow` の欄と1対1か。**
///
/// 欄を足して名前を足し忘れると、その許可はテンプレートで往復しません。
/// `types.rs` を読んで数を突き合わせます。
#[cfg(test)]
mod allow_names_watch {
    use super::ALLOW_NAMES;

    #[test]
    fn every_protect_flag_has_a_name() {
        let src = include_str!("book/types.rs");
        let head = "pub struct ProtectAllow {";
        let from = src.find(head).expect("ProtectAllow が無い");
        let body = &src[from + head.len()..];
        let to = body.find("\n}").expect("終わりが無い");
        let n = body[..to].lines().filter(|l| l.trim().starts_with("pub ")).count();
        assert_eq!(
            ALLOW_NAMES.len(),
            n,
            "ProtectAllow の欄は {n} 個、名前の表は {} 個。\
             足りない欄はテンプレートで往復しません",
            ALLOW_NAMES.len()
        );
    }

    #[test]
    fn the_names_do_not_repeat() {
        for (i, (a, _)) in ALLOW_NAMES.iter().enumerate() {
            for (b, _) in &ALLOW_NAMES[i + 1..] {
                assert_ne!(a, b, "同じ名前が2つある: 「{a}」");
            }
        }
    }
}
