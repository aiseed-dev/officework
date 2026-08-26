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

/// セルの書式を (名前, 項目, 値) の表で持つ
pub mod style;
/// テンプレートの言葉の表(15言語)。**ui/gen_tmpl_words.py が起こします**
pub mod words;

/// 記号を、いまの画面の言語の字にする(書くときに使う)
fn w(sym: &str) -> &'static str {
    words::text(sym)
}

use crate::book::{Book, CellFormat, FreezePane, Pos, ProtectAllow, Sheet};
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
    /// 保護中も許す操作。**記号で持ちます**(書くときに画面の言語へ訳す)
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
    /// **名前つきの書式の定義。** 同じ書式は1つにまとめて名前を付けます
    pub styles: Vec<(String, CellFormat)>,
    /// **どの範囲にどの書式を当てるか。**(シート, 左上, 右下, 書式の名前)
    pub style_at: Vec<(String, Pos, Pos, String)>,
    /// テーマ色の組(12色)
    pub theme: Vec<String>,
    /// 式を R1C1 で見せるか
    pub r1c1: Option<bool>,
}

impl BookTheme {
    /// 持っている物が何も無いか(何も無ければテンプレートを書く意味がない)
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
            && self.style_at.is_empty()
            && self.theme.is_empty()
            && self.r1c1.is_none()
            && self.sheets.iter().all(|s| {
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
    collect_styles(b, &mut t);
    t.theme = b.theme.clone();
    t.r1c1 = b.r1c1.then_some(true);
    t
}

/// **書式を集めて名前を付ける。**
///
/// セル1つに1行を書くと、実物のブックで数千行になります。同じ書式は
/// 1つにまとめ(**型スタンプを作らない** — SEKKEI「書式は数でなく条件で
/// 止める」)、続いている升目は範囲でまとめます。
///
/// 名前は、ブックが名前つきスタイルを持っていればその名前、無ければ
/// `書式1` `書式2` と番号を振ります。**番号は書式の中身から決まる**ので、
/// 同じブックを2度書き出しても同じ名前になります。
fn collect_styles(b: &Book, t: &mut BookTheme) {
    let mut named: Vec<(String, CellFormat)> = Vec::new();
    for (n, _, f) in &b.named_styles {
        named.push((n.clone(), f.clone()));
    }
    for (n, f) in &b.named_styles_new {
        if !named.iter().any(|(m, _)| m == n) {
            named.push((n.clone(), f.clone()));
        }
    }

    // 直に当てた書式を拾い、同じ物は1つにまとめる
    let mut auto: Vec<CellFormat> = Vec::new();
    let name_of = |f: &CellFormat, named: &[(String, CellFormat)], auto: &mut Vec<CellFormat>| {
        if let Some((n, _)) = named.iter().find(|(_, g)| g == f) {
            return n.clone();
        }
        if let Some(i) = auto.iter().position(|g| g == f) {
            return format!("書式{}", i + 1);
        }
        auto.push(f.clone());
        format!("書式{}", auto.len())
    };

    for sh in &b.sheets {
        // **横に続く同じ書式は1つの範囲にまとめます。** 縦のまとめは
        // やっていません — 表の1行が同じ書式という形が事務では多いためです
        let mut run: Option<(Pos, Pos, String)> = None;
        for (p, c) in &sh.cells {
            if c.fmt.is_plain() {
                continue;
            }
            let n = name_of(&c.fmt, &named, &mut auto);
            match &mut run {
                Some((a, z, name))
                    if *name == n && z.row == p.row && z.col + 1 == p.col =>
                {
                    let _ = a;
                    *z = *p;
                }
                _ => {
                    if let Some((a, z, name)) = run.take() {
                        t.style_at.push((sh.name.clone(), a, z, name));
                    }
                    run = Some((*p, *p, n));
                }
            }
        }
        if let Some((a, z, name)) = run.take() {
            t.style_at.push((sh.name.clone(), a, z, name));
        }
        // **`style_of` は読みません。** あれは xlsx の `<c s="…">` の控えで、
        // 原本の styles.xml を据え置くためだけに持っています。画面の書式は
        // `Cell::fmt` が決めており、calc は `style_of` を1度も見ていません。
        // ここで読むと、原本の索引が指す書式がセルに焼き付いてしまいます
    }

    t.styles = named;
    for (i, f) in auto.into_iter().enumerate() {
        t.styles.push((format!("書式{}", i + 1), f));
    }
    // **名前つきスタイルは、どのセルにも当たっていなくても残します。**
    // 利用者が書式の一覧に作った物なので、使っていないという理由で消すと
    // 次に開いたとき無くなっています。落とすのは、この場で番号を振った
    // `書式N` のうち誰にも当たらなかった物だけです
    t.styles.retain(|(n, _)| {
        !n.starts_with(w("format")) || t.style_at.iter().any(|(_, _, _, m)| m == n)
    });
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
    apply_styles(t, b);
    if !t.theme.is_empty() {
        b.theme = t.theme.clone();
    }
    if let Some(v) = t.r1c1 {
        b.r1c1 = v;
    }
}

/// **書式をセルへ当てる。**
///
/// 当てる先が空のセルなら作ります — 罫線だけ引いた升目は、中身が無くても
/// 見た目を持つからです。
fn apply_styles(t: &BookTheme, b: &mut Book) {
    if t.style_at.is_empty() {
        return;
    }
    // 定義は名前つきスタイルとしても持ち越します(画面の一覧に出るため)
    for (n, f) in &t.styles {
        if n.starts_with(w("format")) {
            continue;
        }
        if !b.named_styles.iter().any(|(m, _, _)| m == n)
            && !b.named_styles_new.iter().any(|(m, _)| m == n)
        {
            b.named_styles_new.push((n.clone(), f.clone()));
        }
    }
    for (sheet, a, z, name) in &t.style_at {
        let Some(f) = t.styles.iter().find(|(n, _)| n == name).map(|(_, f)| f.clone()) else {
            continue;
        };
        let Some(s) = b.sheets.iter_mut().find(|s| s.name == *sheet) else { continue };
        for row in a.row..=z.row {
            for col in a.col..=z.col {
                let p = Pos::new(row, col);
                match s.cells.get_mut(&p) {
                    Some(c) => c.fmt = f.clone(),
                    None => {
                        s.set(p, crate::book::Cell { fmt: f.clone(), ..Default::default() });
                    }
                }
            }
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
    if let Some(tb) = style_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = style_at_table(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = book_table(t) {
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




/// 書式の定義。**(名前, 項目, 値)の縦長**です — 欄が 25 あるので横には並べません。
fn style_table(t: &BookTheme) -> Option<Table> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (name, f) in &t.styles {
        for (item, v) in style::to_rows(f) {
            rows.push(vec![name.clone(), w(item).to_string(), v]);
        }
    }
    table(w("format"), &[w("name"), w("item"), w("value")], rows)
}

fn read_style(t: &mut BookTheme, rows: &[Vec<String>]) {
    let mut by_name: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for row in rows {
        let name = pick(row, 0);
        if name.is_empty() {
            continue;
        }
        let item = (pick(row, 1).to_string(), pick(row, 2).to_string());
        match by_name.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => v.push(item),
            None => by_name.push((name.to_string(), vec![item])),
        }
    }
    for (name, items) in by_name {
        let f = style::from_rows(&items);
        match t.styles.iter_mut().find(|(n, _)| *n == name) {
            Some((_, g)) => *g = f,
            None => t.styles.push((name, f)),
        }
    }
}

/// どの範囲にどの書式を当てるか。
fn style_at_table(t: &BookTheme) -> Option<Table> {
    let rows: Vec<Vec<String>> = t
        .style_at
        .iter()
        .map(|(sheet, a, z, name)| {
            let range =
                if a == z { a.a1() } else { format!("{}:{}", a.a1(), z.a1()) };
            vec![sheet.clone(), range, name.clone()]
        })
        .collect();
    table(w("format_applied"), &[w("sheets"), w("range"), w("format")], rows)
}

fn read_style_at(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let sheet = pick(row, 0);
        let range = pick(row, 1);
        let name = pick(row, 2);
        if sheet.is_empty() || range.is_empty() || name.is_empty() {
            continue;
        }
        let (a, z) = match range.split_once(':') {
            Some((a, z)) => (Pos::parse(a.trim()), Pos::parse(z.trim())),
            None => (Pos::parse(range), Pos::parse(range)),
        };
        if let (Some(a), Some(z)) = (a, z) {
            t.style_at.push((sheet.to_string(), a, z, name.to_string()));
        }
    }
}

/// ブック全体の見た目。**シートに紐づかない物**だけ置きます。
fn book_table(t: &BookTheme) -> Option<Table> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    if !t.theme.is_empty() {
        rows.push(vec![w("theme_colors").into(), t.theme.join(",")]);
    }
    if let Some(v) = t.r1c1 {
        rows.push(vec![w("show_r1c1").into(), v.to_string()]);
    }
    table(w("workbook"), &[w("item"), w("value")], rows)
}

fn read_book(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        match words::which(&["theme_colors", "show_r1c1"], pick(row, 0)) {
            Some("theme_colors") => {
                let c: Vec<String> =
                    pick(row, 1).split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                if !c.is_empty() {
                    t.theme = c;
                }
            }
            Some("show_r1c1") => t.r1c1 = read_yes_no(pick(row, 1)),
            _ => {}
        }
    }
}

/// **保護中も許す操作の名前。** 表と `ProtectAllow` の欄を1対1で結びます。
///
/// 名前は Excel の「シートの保護」の小窓の言い方に寄せてあります。
/// **欄を足したらここにも足すこと** — `every_protect_flag_has_a_name` が
/// 数を確かめます。
pub const ALLOW_NAMES: &[(&str, fn(&mut ProtectAllow))] = &[
    ("select_locked_cells", |a| a.select_locked = true),
    ("select_unlocked_cells", |a| a.select_unlocked = true),
    ("format_cells", |a| a.format_cells = true),
    ("format_columns", |a| a.format_cols = true),
    ("format_rows", |a| a.format_rows = true),
    ("insert_columns", |a| a.insert_cols = true),
    ("insert_rows", |a| a.insert_rows = true),
    ("insert_hyperlinks", |a| a.insert_links = true),
    ("delete_columns", |a| a.delete_cols = true),
    ("delete_rows", |a| a.delete_rows = true),
    ("sort_2", |a| a.sort = true),
    ("use_autofilter", |a| a.autofilter = true),
    ("use_pivottable", |a| a.pivot = true),
    ("edit_objects", |a| a.objects = true),
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
        // **どの言語で書かれていても受けます**
        if let Some((_, set)) = ALLOW_NAMES.iter().find(|(sym, _)| sym == n) {
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
        w("view"),
        &[w("sheets"), w("freeze"), w("gridlines"), w("formula_2"), w("rtl"), w("hide"), w("tab_color"), w("default_col_width"), w("default_row_height")],
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
                s.name.clone(), w("row").into(), (r + 1).to_string(),
                lv.to_string(), yes_no(Some(*folded)),
            ]);
        }
        for (c, lv, folded) in &s.col_outline {
            rows.push(vec![
                s.name.clone(), w("tmpl_column").into(), col_name(*c),
                lv.to_string(), yes_no(Some(*folded)),
            ]);
        }
    }
    table(w("tmpl_group"), &[w("sheets"), w("kind"), w("position"), w("level"), w("tmpl_collapsed")], rows)
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
        match words::which(&["row", "tmpl_column"], &kind) {
            Some("row") => {
                if let Ok(n) = at.parse::<u32>() {
                    if n > 0 {
                        s.row_outline.push((n - 1, level, folded));
                    }
                }
            }
            Some("tmpl_column") => {
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
                // 記号を**画面の言語**にして並べます
                s.protect_allow
                    .clone()
                    .unwrap_or_default()
                    .iter()
                    .map(|sym| w(sym))
                    .collect::<Vec<_>>()
                    .join("、"),
            ]
        })
        .collect();
    table(w("tmpl_protect"), &[w("sheets"), w("tmpl_protect"), w("allowed_actions")], rows)
}

fn read_protect(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = pick(row, 0);
        if name.is_empty() {
            continue;
        }
        let on = read_yes_no(pick(row, 1));
        // **どの言語で書かれていても記号に直します。** 知らない字はそのまま
        // 残し(黙って落とさない)、当てるときに飛ばします
        let syms: Vec<&str> = ALLOW_NAMES.iter().map(|(s, _)| *s).collect();
        let names: Vec<String> = pick(row, 2)
            .split(['、', ','])
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .map(|x| words::which(&syms, x).unwrap_or(x).to_string())
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
        w("print"),
        &[w("sheets"), w("scale"), w("fit_to_width"), w("fit_to_height"), w("row_col_headings"), w("title_rows"), w("title_cols")],
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
    table(w("page_break"), &[w("sheets"), w("row"), w("tmpl_column")], rows)
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
            (w("header"), &s.header),
            (w("footer"), &s.footer),
        ] {
            if let Some(x) = v {
                rows.push(vec![s.name.clone(), label.into(), x.clone()]);
            }
        }
        if s.hf_diff_odd_even == Some(true) {
            for (label, v) in [(w("header_even"), &s.header_even), (w("footer_even"), &s.footer_even)] {
                rows.push(vec![s.name.clone(), label.into(), v.clone().unwrap_or_default()]);
            }
        }
        if s.hf_diff_first == Some(true) {
            for (label, v) in [(w("header_first"), &s.header_first), (w("footer_first"), &s.footer_first)] {
                rows.push(vec![s.name.clone(), label.into(), v.clone().unwrap_or_default()]);
            }
        }
    }
    table(w("header_footer"), &[w("sheets"), w("position"), w("tmpl_text")], rows)
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
        const SPOTS: &[&str] = &[
            "header", "footer", "header_even", "footer_even", "header_first", "footer_first",
        ];
        match words::which(SPOTS, &where_at) {
            Some("header") => s.header = Some(text),
            Some("footer") => s.footer = Some(text),
            Some("header_even") => {
                s.header_even = Some(text);
                s.hf_diff_odd_even = Some(true);
            }
            Some("footer_even") => {
                s.footer_even = Some(text);
                s.hf_diff_odd_even = Some(true);
            }
            Some("header_first") => {
                s.header_first = Some(text);
                s.hf_diff_first = Some(true);
            }
            Some("footer_first") => {
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
                    Some(true) => w("landscape_2").into(),
                    Some(false) => w("portrait").into(),
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
    table(w("paper"), &[w("sheets"), w("size"), w("orientation"), w("margins"), w("gridlines"), w("tmpl_zoom")], rows)
}

fn width_table(t: &BookTheme) -> Option<Table> {
    let mut rows = Vec::new();
    for s in &t.sheets {
        for (c, w) in &s.col_width {
            rows.push(vec![s.name.clone(), col_name(*c), numbers(*w)]);
        }
    }
    table(w("col_width"), &[w("sheets"), w("tmpl_column"), w("width_2")], rows)
}

fn height_table(t: &BookTheme) -> Option<Table> {
    let mut rows = Vec::new();
    for s in &t.sheets {
        for (r, h) in &s.row_height {
            rows.push(vec![s.name.clone(), (r + 1).to_string(), numbers(*h)]);
        }
    }
    table(w("row_height"), &[w("sheets"), w("row"), w("height")], rows)
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
        // **どの言語で書かれた題でも受けます。** 配られたテンプレートを
        // 別の国の人が開いても読めないと困るためです(2026-08-26 発注者
        // 「テンプレートは、各国語版が必要です」)
        const TITLES: &[&str] = &[
            "paper", "col_width", "row_height", "print", "page_break", "header_footer",
            "view", "tmpl_group", "tmpl_protect", "format", "format_applied", "workbook",
        ];
        match words::which(TITLES, title) {
            Some("paper") => read_paper(&mut t, body),
            Some("col_width") => read_width(&mut t, body),
            Some("row_height") => read_height(&mut t, body),
            Some("print") => read_print(&mut t, body),
            Some("page_break") => read_break(&mut t, body),
            Some("header_footer") => read_hf(&mut t, body),
            Some("view") => read_view(&mut t, body),
            Some("tmpl_group") => read_outline(&mut t, body),
            Some("tmpl_protect") => read_protect(&mut t, body),
            Some("format") => read_style(&mut t, body),
            Some("format_applied") => read_style_at(&mut t, body),
            Some("workbook") => read_book(&mut t, body),
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
        match words::which(&["landscape_2", "portrait"], pick(row, 2)) {
            Some("landscape_2") => s.landscape = Some(true),
            Some("portrait") => s.landscape = Some(false),
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

/// **テンプレートは各国語版になる**(2026-08-26 発注者)。
///
/// 書くときは画面の言語、読むときはどの言語でも受けます。配られた
/// テンプレートを別の国の人が開いても読めないと困るためです。
#[cfg(test)]
mod language_tests {
    use super::*;
    use crate::book::holes::filled_book;

    fn tmpl_in(lang: &str) -> String {
        crate::font::set_default_language(lang);
        write(&from_book(&filled_book()))
    }

    #[test]
    fn the_template_is_written_in_the_screen_language() {
        let ja = tmpl_in("ja");
        assert!(ja.contains(".用紙"), "日本語の題が出ない:\n{}", &ja[..200.min(ja.len())]);
        let de = tmpl_in("de");
        assert!(de.contains(".Papier"), "ドイツ語の題が出ない:\n{}", &de[..200.min(de.len())]);
        assert!(!de.contains(".用紙"), "ドイツ語なのに日本語の題が残っている");
        crate::font::set_default_language("ja");
    }

    #[test]
    fn a_template_written_in_another_language_still_reads() {
        // ドイツ語で書いて、日本語の画面で読む
        let de = tmpl_in("de");
        crate::font::set_default_language("ja");
        let t = parse(&de).expect("ドイツ語のテンプレートが読めない");
        let want = from_book(&filled_book());
        assert_eq!(t.sheets.len(), want.sheets.len(), "シートの数が合わない");
        let (a, b) = (&want.sheets[0], &t.sheets[0]);
        assert_eq!(b.paper_size, a.paper_size, "用紙の大きさが読めない");
        assert_eq!(b.landscape, a.landscape, "向きが読めない");
        assert_eq!(b.header, a.header, "ヘッダーが読めない");
        assert_eq!(b.protect_allow, a.protect_allow, "許す操作が読めない");
        assert_eq!(t.styles, want.styles, "書式が読めない");
    }

    #[test]
    fn every_language_round_trips() {
        for l in words::LANGS {
            let src = tmpl_in(l);
            crate::font::set_default_language(l);
            let back = parse(&src).unwrap_or_else(|e| panic!("{l}: 読めない: {e}"));
            assert_eq!(back, from_book(&filled_book()), "{l}: 往復で見た目が変わった");
        }
        crate::font::set_default_language("ja");
    }
}

/// **言葉の表が生成のとおりか。**
///
/// `engine/src/booktmpl/words.rs` は `ui/gen_tmpl_words.py` が起こします。
/// 手で直したり、生成し直し忘れたりすると、テンプレートの語が画面の文言と
/// 食い違います。
#[cfg(test)]
mod words_watch {
    use super::words;

    #[test]
    fn every_symbol_the_template_uses_is_in_the_table() {
        // booktmpl.rs と style.rs が `w("…")` と `words::is("…", …)` で呼ぶ記号
        let src = concat!(include_str!("booktmpl.rs"), include_str!("booktmpl/style.rs"));
        let mut want: Vec<&str> = Vec::new();
        for (pat, skip) in [("w(\"", 3), ("words::is(\"", 11), ("words::text(\"", 13)] {
            let mut from = 0;
            while let Some(i) = src[from..].find(pat) {
                let a = from + i + skip;
                match src[a..].find('"') {
                    Some(j) => {
                        want.push(&src[a..a + j]);
                        from = a + j;
                    }
                    None => break,
                }
            }
        }
        // **この試験そのものの字も拾ってしまう**ので、記号の形をした物
        // (小文字と数字と下線)だけを見ます
        want.retain(|s| {
            !s.is_empty()
                && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                && s.starts_with(|c: char| c.is_ascii_lowercase())
        });
        want.sort_unstable();
        want.dedup();
        for sym in want {
            assert!(
                words::WORDS.iter().any(|(k, _)| *k == sym),
                "記号「{sym}」が言葉の表に無い。\
                 ui/gen_tmpl_words.py の WORDS に足して生成し直してください"
            );
        }
    }

    /// **表に並べてある記号も確かめます。** `w("…")` と書かずに表へ入れた
    /// 記号(書式の項目・線種・揃え・許す操作)は、呼び出しの形を探すだけの
    /// 見張りでは見つかりません。表に無い記号は、そのまま見出しに出ます
    /// (2026-08-26 に `font_2` が出た)。
    #[test]
    fn the_symbols_in_the_lists_are_in_the_table() {
        let mut all = super::style::symbols();
        all.extend(super::ALLOW_NAMES.iter().map(|(s, _)| *s));
        for sym in all {
            assert!(
                words::WORDS.iter().any(|(k, _)| *k == sym),
                "記号「{sym}」が言葉の表に無い。テンプレートに記号がそのまま出ます"
            );
        }
    }

    #[test]
    fn the_table_has_fifteen_languages() {
        assert_eq!(words::LANGS.len(), 15, "言語の数が変わりました");
        assert!(words::LANGS.contains(&"ja") && words::LANGS.contains(&"en"));
    }

    /// **同じ字が2つの記号を指していないか。** 指していると読むときに
    /// 取り違えます。呼ぶ側は [`words::which`] に「この場所に来る記号」を
    /// 渡すので、**同じ場所に来る記号どうし**でぶつからなければ構いません。
    #[test]
    fn words_in_the_same_place_do_not_collide() {
        const PLACES: &[&[&str]] = &[
            &["paper", "col_width", "row_height", "print", "page_break", "header_footer",
              "view", "tmpl_group", "tmpl_protect", "format", "format_applied", "workbook"],
            &["landscape_2", "portrait"],
            &["header", "footer", "header_even", "footer_even", "header_first", "footer_first"],
            &["row", "tmpl_column"],
            &["theme_colors", "show_r1c1"],
            &["edge_top", "edge_bottom", "edge_left", "edge_right"],
            &["align_general", "left", "center", "right", "justify", "center_across", "distributed"],
            &["top", "center", "bottom", "distributed"],
            &["hairline", "dotted", "dash_dot_dot", "dash_dot", "dashed", "thin",
              "medium_dash_dot_dot", "medium_dash_dot", "medium_dashed", "medium",
              "thick", "double", "slant_dash_dot"],
        ];
        for place in PLACES {
            for (i, a) in place.iter().enumerate() {
                let Some((_, ta)) = words::WORDS.iter().find(|(k, _)| k == a) else {
                    panic!("記号「{a}」が表に無い")
                };
                for b in &place[i + 1..] {
                    let Some((_, tb)) = words::WORDS.iter().find(|(k, _)| k == b) else {
                        panic!("記号「{b}」が表に無い")
                    };
                    for (n, (x, y)) in words::LANGS.iter().zip(ta.iter().zip(tb.iter())) {
                        assert_ne!(x, y, "{n} で「{a}」と「{b}」が同じ字「{x}」になっています");
                    }
                }
            }
        }
    }
}
