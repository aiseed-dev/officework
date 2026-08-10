//! main.rs からの純移動(2026-08-06 の分割)。挙動は変えない。

use crate::*;

/// 本文のフォント。**同梱せず、システムから探す**
/// (埋め込むと実行ファイルがフォントを配ることになり、免許の表示義務も付く)。
///
/// 起動時に一度だけ読み、以後は借りて使う。
/// 見つからなければ**その場で止める** — 日本語が豆腐になった画面を
/// 「動いている」と見せない。
pub(crate) fn font_data() -> &'static [u8] {
    static FONT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        {
            // 文書が書体を指定していればそれを、無ければ機械にある日本語フォントを
            let (fam, _) = kumihan::font::for_document(None).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });
            kumihan::font::load(fam).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            })
        }
    })
}

pub(crate) const ROW_H: f32 = 24.0;
/// `RRGGBB` を色にする。読めなければ黒
/// 下地に選択の緑を混ぜる。**塗りを置き換えない** — 選択中も帳票本来の色が
/// 透けて見える(選択を解かないと色が確かめられない、を避ける)。
pub(crate) fn tint(base: gpui::Rgba, k: f32) -> gpui::Rgba {
    let accent = (0x1B as f32 / 255.0, 0x6E as f32 / 255.0, 0x3C as f32 / 255.0);
    gpui::Rgba {
        r: base.r * (1.0 - k) + accent.0 * k,
        g: base.g * (1.0 - k) + accent.1 * k,
        b: base.b * (1.0 - k) + accent.2 * k,
        a: 1.0,
    }
}

pub(crate) fn hex(s: &str) -> gpui::Rgba {
    let g = |i: usize| {
        s.get(i * 2..i * 2 + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .map(|v| v as f32 / 255.0)
            .unwrap_or(0.0)
    };
    gpui::Rgba { r: g(0), g: g(1), b: g(2), a: 1.0 }
}

pub(crate) const COL_W: f32 = 108.0;

/// 定数表の1行 — [`ui::item!`] の組に中身をくっつけて平らな3つ組にする。
///
/// 表は `(鍵, 見出し, 中身)`。**引き当ては鍵**(日本語のまま)、画面は見出し。
/// これで日本語のリテラルは表に1度きり — 鍵と見出しがずれる余地が無い。
/// 表が `const` でなく `fn` なのは、見出しが実行時に決まる(言語は起動時に
/// 読む)ため。どれも項が十数個の表なので、開くたびに作って構わない。
pub(crate) fn row<T>(
    (key, label): (&'static str, &'static str),
    v: T,
) -> (&'static str, &'static str, T) {
    (key, label, v)
}

/// 一覧の項を [`ui::item!`] の組から作る(鍵はそのまま、見出しは訳)。
///
///     self.pick = Some((menu(&[ui::item!(…), ui::item!(…)]), at));
pub(crate) fn menu(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items.iter().map(|(k, l)| (k.to_string(), l.to_string())).collect()
}

/// 一覧の項を**値そのもの**から作る(鍵=見出し)。
///
/// 書体名・ファイル名・シート名・定義した名前など、**画面の文言ではなく
/// 中身**の一覧に使う。こういう字は訳さない — 訳したら別物を指してしまう。
/// 訳すべき見出しは [`ui::item!`] のほうで作る。
pub(crate) fn plain<I, S>(items: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    items
        .into_iter()
        .map(|s| {
            let s = s.into();
            (s.clone(), s)
        })
        .collect()
}
/// xlsx の列幅1(=「0」1個ぶん)を何画素にするか。既定幅 8.43 ≒ 108px の比
pub(crate) const PX_PER_CHW: f32 = 108.0 / 8.43;
/// **文字が要る幅(px)。** 半角=1・全角=2 で数えた概算。
///
/// 画面のはみ出し描き(隣の空きセルへ流す判定)と、列幅の自動調整で
/// **同じ物差しを使う**。別々に測ると「自動調整したのにまだはみ出す」に
/// なる。厳密な字送りではないが、両方が同じだけずれるので破綻しない。
pub(crate) fn text_px(text: &str, size_px: f32) -> f32 {
    let units: f32 = text
        .chars()
        .map(|ch| if (ch as u32) < 0x2E80 { 1.0 } else { 2.0 })
        .sum();
    units * size_px * 0.52 + 14.0
}

/// 記号の組(分類名, 文字たち)。**帳票で本当に打つものを先に**。
/// 本家は Unicode の「範囲」で切るが、それは字典の切り方で、
/// 帳票を書く人の探し方ではない(通貨より先に 〒 や ㈱ が要る)
pub(crate) fn symbol_groups() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        row(ui::item!("帳票でよく使う"), "〒℡№㈱㈲〆※‰°′″"),
        row(ui::item!("しるし"), "○●◎△▲▽▼□■◇◆☆★×✓☑☐"),
        row(ui::item!("矢印"), "→←↑↓⇒⇐⇔↔↗↘↙↖"),
        row(ui::item!("丸数字"), "①②③④⑤⑥⑦⑧⑨⑩⑪⑫"),
        row(ui::item!("通貨"), "¥＄€£¢₩₹"),
        row(ui::item!("算術"), "±×÷≠≒≦≧∞√∑∫"),
        row(ui::item!("かっこ・区切り"), "「」『』【】〔〕・…‥※〜"),
        row(ui::item!("ギリシャ"), "αβγδεζηθλμπστφω"),
    ]
}

/// リボンから開く一覧の幅。書体名は長いので、セルの列幅ではなくこの幅。
pub(crate) const POP_W: f32 = 240.0;

/// **リボンのボタンから開く一覧を、そのボタンの真下に出す。**
///
/// リボンで書体を変えようとすると、一覧が押したボタンではなく**選んで
/// いるセルの下**に出ていた(発注者報告 2026-08-08)。ボタンは画面の
/// 一番上、一覧は画面の真ん中 — 目が二往復する。
///
/// `btn`・`pane` はどちらも窓の座標での (x, y, 幅, 高さ)。返す値は
/// **格子の面を基準にした座標**(一覧はその面の中に置かれるため)。
///
/// 横はボタンの左端にそろえ、右端からはみ出すぶんだけ内へ寄せる。幅は
/// 中身でまちまちなので POP_W で見る。面の幅がまだ分からない(一度も
/// 描いていない)ときは寄せない。
///
/// **縦は面の一番上まで。** リボンは面より上にあるので本当はボタンの
/// 真下に出したいが、一覧を置く層は格子の面の中にあり `overflow_hidden`
/// で切られる。gpui の `deferred` で層ごと外に出す手を試したが、
/// **一覧が一つも描かれなくなった**ので戻した(2026-08-08)。面より上へ
/// 出すには一覧の層を窓の根に移す必要があり、それは別途。
pub(crate) fn pop_under(btn: (f32, f32, f32, f32), pane: (f32, f32, f32, f32)) -> (f32, f32) {
    let (bx, by, _, bh) = btn;
    let (px0, py0, pw, _) = pane;
    let x = bx - px0;
    let x = if pw > POP_W { x.min(pw - POP_W) } else { x };
    // ボタンの下辺から 2px 空ける。面の中に収まらなければ面の一番上
    (x.max(0.0), (by + bh + 2.0 - py0).max(2.0))
}

/// ボタンの場所がまだ分からないとき(描く前に鍵で呼ばれた等)の逃げ道。
/// 押した点を左端と見なして同じように寄せる。
pub(crate) fn pop_at_click(click_x: f32, pane: (f32, f32, f32, f32)) -> (f32, f32) {
    pop_under((click_x - 12.0, pane.1 - 2.0, 0.0, 0.0), pane)
}

/// 描く行の並び。固定行は常に頭に、残りは窓から。
pub(crate) fn grid_rows(frozen: Option<Pos>, view: Pos, n: u32) -> Vec<u32> {
    let f = frozen.map(|p| p.row).unwrap_or(0);
    let mut out: Vec<u32> = (0..f.min(n)).collect();
    let start = view.row.max(f);
    while (out.len() as u32) < n {
        let next = start + out.len() as u32 - f.min(n);
        out.push(next);
    }
    out
}

pub(crate) fn grid_cols(frozen: Option<Pos>, view: Pos, n: u32) -> Vec<u32> {
    let f = frozen.map(|p| p.col).unwrap_or(0);
    let mut out: Vec<u32> = (0..f.min(n)).collect();
    let start = view.col.max(f);
    while (out.len() as u32) < n {
        let next = start + out.len() as u32 - f.min(n);
        out.push(next);
    }
    out
}

pub(crate) const HEAD_W: f32 = 46.0;
pub(crate) const ROWS: u32 = 30;
pub(crate) const COLS: u32 = 9;

/// 境界の取っ手の当たり幅(縁から前後この px 以内で掴める)。
/// 見出しのクリックに他の意味は無いので、広めに取って掴みやすくする
pub(crate) const GRIP: f32 = 5.0;

/// `start` から `sizes` の幅で並ぶ区分のうち、`pos` がどの区分の
/// 右端(下端)±GRIP に掛かるかを返す。列見出し・行見出しの境界の当たり判定。
pub(crate) fn grip_hit(sizes: &[(u32, f32)], start: f32, pos: f32) -> Option<u32> {
    let mut edge = start;
    for (i, w) in sizes {
        edge += w;
        if (pos - edge).abs() <= GRIP {
            return Some(*i);
        }
    }
    None
}

/// `start` から `sizes` の幅で並ぶ区分のうち、`pos` がどの区分の中に
/// 入るかを返す。見出しのクリック(列・行の選択)の当たり判定。
pub(crate) fn index_at(sizes: &[(u32, f32)], start: f32, pos: f32) -> Option<u32> {
    let mut x = start;
    for (i, w) in sizes {
        if pos >= x && pos < x + w {
            return Some(*i);
        }
        x += w;
    }
    None
}

/// 見出しの境界を掴んだドラッグ(列幅・行高を変える)
pub(crate) struct SizeDrag {
    /// 列か(false なら行)
    pub(crate) col: bool,
    pub(crate) idx: u32,
    /// 掴んだ位置(px。列なら x、行なら y)
    pub(crate) grab: f32,
    /// 掴んだときの大きさ(px)
    pub(crate) base: f32,
    /// 動かしたか。**最初に動いた瞬間に undo の控えを取る** —
    /// 掴んだだけ(クリック)で redo の控えが消えるのを防ぐ
    pub(crate) moved: bool,
}

/// 使われていないシート名(Sheet2, Sheet3, …)。
pub(crate) fn unique_sheet_name(book: &Book) -> String {
    let mut n = book.sheets.len() + 1;
    loop {
        let name = format!("Sheet{n}");
        if !book.sheets.iter().any(|s| s.name == name) {
            return name;
        }
        n += 1;
    }
}

/// 複製のシート名(Excel の流儀: 「名前 (2)」から空きを探す)
pub(crate) fn copy_sheet_name(book: &Book, base: &str) -> String {
    let mut n = 2;
    loop {
        let name = format!("{base} ({n})");
        if !book.sheets.iter().any(|s| s.name == name) {
            return name;
        }
        n += 1;
    }
}

/// 式の文字列の外側だけで、古いシート名の参照(`古!` と `'古'!`)を
/// 新しい名前に書き換える。変えたら Some(新しい式)。
/// 名前の頭が別の語の続きのとき(例: 「合計!」の中の「計!」)は書き換えない
pub(crate) fn rename_refs_in(f: &str, old: &str, new: &str) -> Option<String> {
    let needs_quote =
        |n: &str| !n.chars().all(|c| c.is_alphanumeric() || c == '_') || n.is_empty();
    let to = if needs_quote(new) { format!("'{new}'!") } else { format!("{new}!") };
    let bare = format!("{old}!");
    let quoted = format!("'{old}'!");
    let cs: Vec<char> = f.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut changed = false;
    let mut in_str = false;
    while i < cs.len() {
        let c = cs[i];
        if c == '"' {
            in_str = !in_str;
            out.push(c);
            i += 1;
            continue;
        }
        if !in_str {
            let rest: String = cs[i..].iter().collect();
            let prev_word = i > 0 && (cs[i - 1].is_alphanumeric() || cs[i - 1] == '_');
            if rest.starts_with(&quoted) {
                out.push_str(&to);
                i += quoted.chars().count();
                changed = true;
                continue;
            }
            if !prev_word && rest.starts_with(&bare) {
                out.push_str(&to);
                i += bare.chars().count();
                changed = true;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    changed.then_some(out)
}

/// 全シートの式と名前の定義の中のシート参照を、新しい名前へ書き換える。
/// 書き換えた式の数を返す(黙って直さない — 状態行で件数を言う)
pub(crate) fn rename_sheet_refs(book: &mut Book, old: &str, new: &str) -> usize {
    let mut n = 0;
    for s in book.sheets.iter_mut() {
        let hits: Vec<(Pos, String)> = s
            .cells
            .iter()
            .filter_map(|(p, c)| {
                c.formula
                    .as_ref()
                    .and_then(|f| rename_refs_in(f, old, new))
                    .map(|nf| (*p, nf))
            })
            .collect();
        for (p, nf) in hits {
            if let Some(c) = s.cells.get_mut(&p) {
                c.formula = Some(nf);
                n += 1;
            }
        }
        for (_, r) in s.names.iter_mut() {
            if let Some(nr) = rename_refs_in(r, old, new) {
                *r = nr;
            }
        }
    }
    n
}

/// 式の**文字列の中**に古いシート名の参照が残っている数を数える。
/// 改名では文字列の中は書き換えない(INDIRECT は「動かない参照」を作る
/// 道具で、Excel も追随させない)— だから**黙って壊さず、件数を言う**
pub(crate) fn stale_in_strings(book: &Book, old: &str) -> usize {
    let bare = format!("{old}!");
    let quoted = format!("'{old}'!");
    let hit = |f: &str| -> bool {
        let mut in_str = false;
        let cs: Vec<char> = f.chars().collect();
        let mut i = 0;
        while i < cs.len() {
            if cs[i] == '"' {
                in_str = !in_str;
                i += 1;
                continue;
            }
            if in_str {
                let rest: String = cs[i..].iter().collect();
                if rest.starts_with(&bare) || rest.starts_with(&quoted) {
                    // 名前の頭が別の語の続きなら別物(「合計!」の中の「計!」)
                    let prev_word =
                        i > 0 && (cs[i - 1].is_alphanumeric() || cs[i - 1] == '_');
                    if !prev_word {
                        return true;
                    }
                }
            }
            i += 1;
        }
        false
    };
    book.sheets
        .iter()
        .flat_map(|s| s.cells.values())
        .filter(|c| c.formula.as_deref().is_some_and(hit))
        .count()
}

/// 選んだ範囲を TSV(タブ区切り・行は改行)にする。
/// 式は `=` のまま持つ — 表計算どうしの受け渡しの通り相場。
pub(crate) fn range_tsv(s: &sheet::Sheet, a: Pos, b: Pos) -> String {
    (a.row..=b.row)
        .map(|r| {
            (a.col..=b.col)
                .map(|c| s.get(Pos::new(r, c)).map(|x| x.editable()).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// TSV を格子に戻す。他のアプリから来るもの(\r\n・末尾の改行)も受ける。
pub(crate) fn tsv_grid(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_suffix('\n').unwrap_or(text);
    text.split('\n')
        .map(|line| {
            line.trim_end_matches('\r')
                .split('\t')
                .map(|s| s.to_string())
                .collect()
        })
        .collect()
}

/// 行と列を入れ替える(転置)。歯抜けは空欄として埋める。
pub(crate) fn transpose<T: Clone + Default>(g: &[Vec<T>]) -> Vec<Vec<T>> {
    let rows = g.len();
    let cols = g.iter().map(|r| r.len()).max().unwrap_or(0);
    (0..cols)
        .map(|c| {
            (0..rows)
                .map(|r| g[r].get(c).cloned().unwrap_or_default())
                .collect()
        })
        .collect()
}

/// 控えたセルの**値だけ**を流し込む(式は計算結果の値になる)。書式は据え置き。
/// 控えの空セルは中身を消す(書式は残す)— 空も「値」のうち。
pub(crate) fn paste_values_cells(s: &mut sheet::Sheet, at: Pos, cells: &[Vec<Option<Cell>>]) -> usize {
    let mut n = 0usize;
    for (dr, row) in cells.iter().enumerate() {
        for (dc, src) in row.iter().enumerate() {
            let p = Pos::new(at.row + dr as u32, at.col + dc as u32);
            let fmt = s.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
            let value = src.as_ref().map(|c| c.value.clone()).unwrap_or(Value::Empty);
            s.set(p, Cell { formula: None, value, fmt });
            n += 1;
        }
    }
    n
}

/// 外から来た TSV の**値だけ**を流し込む。`=` で始まる欄は式にせず文字として置く
/// (外の式は計算できない — 黙って別の意味にしない)。
pub(crate) fn paste_values_text(s: &mut sheet::Sheet, at: Pos, grid: &[Vec<String>]) -> usize {
    let mut n = 0usize;
    for (dr, row) in grid.iter().enumerate() {
        for (dc, text) in row.iter().enumerate() {
            let p = Pos::new(at.row + dr as u32, at.col + dc as u32);
            let fmt = s.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
            let mut cell = if text.starts_with('=') {
                Cell { formula: None, value: Value::Text(text.clone()), fmt: Default::default() }
            } else {
                Cell::input(text)
            };
            cell.fmt = fmt;
            s.set(p, cell);
            n += 1;
        }
    }
    n
}

/// 「関数を挿入」の小窓(本家の FormulaDialog と同じ形 —
/// 検索 / 分類 / 一覧 / 引数と説明 / OK・キャンセル)。
/// 一覧・引数・説明は funcs.rs(本家の日本語から生成。使える関数だけ)
pub(crate) struct FnDlg {
    pub(crate) search: Editor,
    /// FN_GROUPS の添字(0 = すべて)
    pub(crate) group: usize,
    /// 絞り込み後の一覧の中の選択
    pub(crate) sel: usize,
}

/// 分類の耳。「すべて」+ funcs.rs の分類
pub(crate) const FN_GROUPS: &[&str] = &["すべて", "数学", "統計", "文字列", "論理", "日付", "検索", "財務", "情報"];

/// 「関数の引数」の画面(本家の第2段)。引数ごとの欄と説明、結果の下見
pub(crate) struct FnArgs {
    pub(crate) f: &'static funcs::FnInfo,
    /// (引数名, 省略可)
    pub(crate) names: Vec<(String, bool)>,
    pub(crate) eds: Vec<Editor>,
    pub(crate) focus: usize,
    /// 関数の結果(引数を打つたびに、表の複製で計算した下見)
    pub(crate) result: String,
    /// セルの掴みの起点。ドラッグすると「起点:いま」の範囲が欄に入る
    pub(crate) pick_from: Option<Pos>,
}

/// 引数の書き方「(数値1, [数値2], ...)」を(名前, 省略可)の列に解く
pub(crate) fn parse_fn_args(spec: &str) -> Vec<(String, bool)> {
    spec.trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "...")
        .map(|s| {
            let opt = s.starts_with('[');
            (s.trim_start_matches('[').trim_end_matches(']').to_string(), opt)
        })
        .collect()
}

/// 検索と分類で絞った一覧(名前順は funcs.rs の並びのまま)
pub(crate) fn fn_filtered(search: &str, group: usize) -> Vec<&'static funcs::FnInfo> {
    let q = search.trim().to_uppercase();
    funcs::FUNCS
        .iter()
        .filter(|f| group == 0 || f.group == FN_GROUPS[group])
        .filter(|f| q.is_empty() || f.name.contains(&q))
        .collect()
}

/// ソルバーの小窓(ONLYOFFICE の「ソルバーのパラメータ」と同じ形)。
/// 解法は単体法 LP だけ — 本家と同じで、非線形は正直に断る。
pub(crate) struct Solver {
    /// 目的のセル
    pub(crate) target: Editor,
    /// 0=最大 1=最小 2=値
    pub(crate) mode: u8,
    /// mode=2 の目標の値
    pub(crate) value: Editor,
    /// 変数セル(範囲・カンマ区切り)
    pub(crate) vars: Editor,
    /// 決めた制約(左辺セル/範囲, 記号, 右辺)
    pub(crate) cons: Vec<(String, &'static str, String)>,
    /// 追加・変更中の制約の入力
    pub(crate) con_l: Editor,
    pub(crate) con_op: usize,
    pub(crate) con_r: Editor,
    /// 制約のない変数を非負にする
    pub(crate) nonneg: bool,
    /// 打鍵の宛先: 0=目的 1=値 2=変数 3=制約左 4=制約右
    pub(crate) focus: u8,
    /// 一覧で選んだ制約(変更・削除の相手)
    pub(crate) sel: Option<usize>,
}

impl Solver {
    pub(crate) fn new(target: &str) -> Self {
        Solver {
            target: Editor::new(target),
            mode: 0,
            value: Editor::new(""),
            vars: Editor::new(""),
            cons: Vec::new(),
            con_l: Editor::new(""),
            con_op: 0,
            con_r: Editor::new(""),
            nonneg: true,
            focus: 2, // まず変数セルを聞く(目的は選択から入っている)
            sel: None,
        }
    }
    pub(crate) fn focused(&mut self) -> &mut Editor {
        match self.focus {
            0 => &mut self.target,
            1 => &mut self.value,
            2 => &mut self.vars,
            3 => &mut self.con_l,
            _ => &mut self.con_r,
        }
    }
    pub(crate) fn focused_ref(&self) -> &Editor {
        match self.focus {
            0 => &self.target,
            1 => &self.value,
            2 => &self.vars,
            3 => &self.con_l,
            _ => &self.con_r,
        }
    }
}

pub(crate) const SOLVER_OPS: [&str; 3] = ["<=", "=", ">="];

/// 「データの入力規則」のパネル(本家の3タブのダイアログと同じ形 —
/// 設定 / メッセージを入力 / エラー警告、OK・キャンセル)。
pub(crate) struct DvDlg {
    /// 0=設定 1=メッセージを入力 2=エラー警告
    pub(crate) tab: u8,
    /// 許可: dv_kinds() の添字(0 すべての値 / 1 整数 / 2 小数 /
    /// 3 リスト / 4 文字列の長さ)。読めない種類(日付など)を開いたら 5=そのまま
    pub(crate) kind: usize,
    /// データ: dv_ops() の添字(整数/小数/文字数のときだけ)
    pub(crate) op: usize,
    /// 空白を無視(xlsx の allowBlank)
    pub(crate) allow_blank: bool,
    /// これらの変更を同じ設定の他のすべてのセルに適用する
    pub(crate) apply_same: bool,
    /// セルの ▾ を出さない(xlsx の showDropDown="1")
    pub(crate) hide_arrow: bool,
    /// エラー警告のスタイル: 0 停止 / 1 警告 / 2 情報
    pub(crate) err_style: usize,
    /// 欄: 0=最小(値・元の値) 1=最大 2=メッセージ題 3=メッセージ本文
    /// 4=エラー題 5=エラー本文
    pub(crate) eds: [Editor; 6],
    /// 打鍵の宛先(eds の添字)
    pub(crate) focus: usize,
    /// 開いているドロップダウン: 0 なし / 1 許可 / 2 データ / 3 スタイル
    pub(crate) menu: u8,
    /// 種類が読めない既存の規則(日付・時刻・カスタム)。OK でもそのまま保つ
    pub(crate) keep: Option<sheet::model::Validation>,
    /// 開いたときの既存の規則(「同じ設定の他のセル」の比較の相手)
    pub(crate) was: Option<sheet::model::Validation>,
}

impl DvDlg {
    pub(crate) fn focused(&mut self) -> &mut Editor {
        &mut self.eds[self.focus.min(5)]
    }
    pub(crate) fn focused_ref(&self) -> &Editor {
        &self.eds[self.focus.min(5)]
    }
}

/// 許可の一覧(判定できる種類だけ。日付・時刻・カスタムは保持のみ)。
/// **見出しだけの表** — 引き当ては添字で、`DV_KIND_XLSX` と並びが対。
/// 並びを変えるときは必ず両方いっしょに
pub(crate) fn dv_kinds() -> [&'static str; 5] {
    [
        ui::t!("すべての値"),
        ui::t!("整数"),
        ui::t!("小数"),
        ui::t!("リスト"),
        ui::t!("文字列の長さ"),
    ]
}
/// kind の添字 → xlsx の type
pub(crate) const DV_KIND_XLSX: [&str; 5] = ["", "whole", "decimal", "list", "textLength"];
/// データ(比較)の一覧 `(xlsx の operator, 見出し)`。並びは xlsx の operator と対。
/// **引き当ては operator**(訳さない字)、画面は見出し
pub(crate) fn dv_ops() -> [(&'static str, &'static str); 8] {
    [
        ("between", ui::t!("次の値の間")),
        ("notBetween", ui::t!("次の値の間以外")),
        ("equal", ui::t!("次の値に等しい")),
        ("notEqual", ui::t!("次の値に等しくない")),
        ("greaterThan", ui::t!("次の値より大きい")),
        ("lessThan", ui::t!("次の値より小さい")),
        ("greaterThanOrEqual", ui::t!("次の値より大きいか等しい")),
        ("lessThanOrEqual", ui::t!("次の値より小さいか等しい")),
    ]
}
/// エラー警告のスタイル `(xlsx の errorStyle, 見出し)`。引き当ては errorStyle
pub(crate) fn dv_styles() -> [(&'static str, &'static str); 3] {
    [
        ("stop", ui::t!("停止")),
        ("warning", ui::t!("警告")),
        ("information", ui::t!("情報")),
    ]
}

/// SmartArt の一覧。**分類・並び・名前は Euro-Office の現物**
/// (web-apps の define.js の並びと ja.json の訳)から取った。
/// 載せるのは**うちの図形(SVG 方式)で組めるものだけ** —
/// できないものを、できるように見せない。
/// 分類は (鍵, 見出し, その中の形たち)。形も (鍵, 見出し, 図形の種類)。
/// **引き当ては鍵**(日本語) — 見出しだけが画面の言語になる
#[allow(clippy::type_complexity)]
pub(crate) fn smartart(
) -> Vec<(&'static str, &'static str, Vec<(&'static str, &'static str, &'static str)>)> {
    vec![
        row(ui::item!("リスト"), vec![
            row(ui::item!("カード型リスト"), "block-list"),
            row(ui::item!("縦方向リスト"), "vbox-list"),
            row(ui::item!("ピラミッドのリスト"), "pyramid-list"),
        ]),
        row(ui::item!("プロセス"), vec![
            row(ui::item!("基本ステップ"), "basic-process"),
            row(ui::item!("プロセス"), "chevron-process"),
            row(ui::item!("タイムライン"), "timeline"),
        ]),
        row(ui::item!("循環"), vec![
            row(ui::item!("基本の循環"), "basic-cycle"),
            row(ui::item!("ボックス循環"), "block-cycle"),
        ]),
        row(ui::item!("階層"), vec![
            row(ui::item!("組織図"), "org-chart"),
            row(ui::item!("階層"), "hierarchy"),
        ]),
        row(ui::item!("関係"), vec![row(ui::item!("基本ベン図"), "venn")]),
        row(ui::item!("マトリックス"), vec![row(ui::item!("基本マトリックス"), "matrix")]),
        row(ui::item!("ピラミッド"), vec![row(ui::item!("基本ピラミッド"), "pyramid")]),
    ]
}

/// セル・範囲の列挙を読む(A1 / B2:B5 / $A$1。カンマ・読点・空白区切り)。
/// 範囲は左上→右下に展開する。読めない・大きすぎるときは None。
pub(crate) fn parse_cell_list(text: &str, cap: usize) -> Option<Vec<Pos>> {
    let mut out = Vec::new();
    // $ の絶対参照の印は捨て、小文字も受ける(Excel と同じく区別しない)
    for tok in split_fields(&text.replace('$', "").to_uppercase()) {
        if let Some((a, b)) = tok.split_once(':') {
            let (a, b) = (Pos::parse(a.trim())?, Pos::parse(b.trim())?);
            let (r0, r1) = (a.row.min(b.row), a.row.max(b.row));
            let (c0, c1) = (a.col.min(b.col), a.col.max(b.col));
            for r in r0..=r1 {
                for c in c0..=c1 {
                    out.push(Pos::new(r, c));
                    if out.len() > cap {
                        return None;
                    }
                }
            }
        } else {
            out.push(Pos::parse(tok.trim())?);
            if out.len() > cap {
                return None;
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// ピボットの聞き取りの途中経過。パネルを3枚続けて使う間の控え
/// (行に並べる欄 → 列に広げる欄 → 値と集計、の順に聞く)。
pub(crate) struct PivotPend {
    pub(crate) a: Pos,
    pub(crate) b: Pos,
    pub(crate) headers: Vec<String>,
    pub(crate) rows_sel: Vec<String>,
    pub(crate) cols_sel: Vec<String>,
    /// 値にする見出し(集計の選択へ渡す控え)
    pub(crate) val_sel: String,
    /// 組み替えの相手(book.pivots の番号)。None = 新しく挿入
    pub(crate) replace: Option<usize>,
}

/// 見出しの列挙を割る(カンマ・読点・セミコロン・空白のどれでも。
/// ; も受けるのは日本語配列で : が ; に化けやすいため)。
pub(crate) fn split_fields(text: &str) -> Vec<String> {
    text.split(|c: char| matches!(c, ',' | '、' | ';' | '；') || c.is_whitespace())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// 集計のしかたの一覧 `(鍵, 見出し)`。**鍵はピボットの定義に書き込む字** —
/// 訳した字を書き込むと別物を指すので、鍵は日本語のまま。画面は見出しだけ
pub(crate) fn pivot_aggs() -> Vec<(&'static str, &'static str)> {
    vec![
        ui::item!("合計"),
        ui::item!("平均"),
        ui::item!("個数"),
        ui::item!("最大"),
        ui::item!("最小"),
    ]
}


/// ピボットの指図を JSON にする(手で組む — グラフと同じ割り切り)。
pub(crate) fn pivot_spec_json(headers: &[String], rows: &[Vec<String>], d: &sheet::model::PivotDef) -> String {
    let esc = |t: &str| t.replace('\\', "\\\\").replace('"', "\\\"");
    let strs = |xs: &[String]| {
        xs.iter().map(|x| format!("\"{}\"", esc(x))).collect::<Vec<_>>().join(",")
    };
    let hides = d
        .hide
        .iter()
        .map(|(f, vs)| format!("[\"{}\",[{}]]", esc(f), strs(vs)))
        .collect::<Vec<_>>()
        .join(",");
    let vf = match &d.vfilter {
        Some((op, th)) => format!("[\"{}\",{th}]", esc(op)),
        None => "null".into(),
    };
    let groups = d
        .group_by
        .iter()
        .map(|(f, unit)| format!("[\"{}\",\"{}\"]", esc(f), esc(unit)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"headers\":[{}],\"rows\":[{}],\"index\":[{}],\"columns\":[{}],\"value\":\"{}\",\"agg\":\"{}\",\"totals\":{},\"subtotals\":{},\"blank_rows\":{},\"compact\":{},\"hide\":[{hides}],\"vfilter\":{vf},\"group\":[{groups}],\"show_as\":\"{sa}\"}}",
        strs(headers),
        rows.iter().map(|r| format!("[{}]", strs(r))).collect::<Vec<_>>().join(","),
        strs(&d.rows_sel),
        strs(&d.cols_sel),
        esc(&d.value),
        esc(&d.agg),
        d.totals,
        d.subtotals,
        d.blank_rows,
        d.compact,
        sa = esc(&d.show_as),
    )
}

/// ピボットの台本の答えを読む。各行の1欄目は種別
/// (h=見出し d=データ s=小計 b=空行 t=総計)、残りが欄。
pub(crate) fn parse_pivot_grid(raw: &str) -> (Vec<Vec<String>>, Vec<char>) {
    let mut grid = Vec::new();
    let mut kinds = Vec::new();
    for line in raw.split('\u{1e}') {
        let mut it = line.split('\u{1f}');
        let kind = it.next().and_then(|k| k.chars().next()).unwrap_or('d');
        grid.push(it.map(|v| v.to_string()).collect());
        kinds.push(kind);
    }
    (grid, kinds)
}

/// 表のデザインの「合計行」。選択の下の行に、数の列へ =SUM(…) を入れて
/// 太字+上罫線にする。1行目が見出し(文字)なら合計の範囲から外す。
/// 文字の列の先頭には「合計」の札。書いた欄の数を返す。
pub(crate) fn add_total_row(s: &mut sheet::Sheet, a: Pos, b: Pos) -> usize {
    let header = (a.col..=b.col).any(|c| {
        matches!(s.get(Pos::new(a.row, c)).map(|x| &x.value), Some(Value::Text(_)))
    });
    let from = if header && b.row > a.row { a.row + 1 } else { a.row };
    let total = b.row + 1;
    let mut n = 0usize;
    for c in a.col..=b.col {
        let numeric = (from..=b.row).any(|r| {
            matches!(s.get(Pos::new(r, c)).map(|x| &x.value), Some(Value::Number(_)))
        });
        let p = Pos::new(total, c);
        let fmt0 = s.get(p).map(|x| x.fmt.clone()).unwrap_or_default();
        let mut cell = if numeric {
            Cell::input(&format!(
                "=SUM({}:{})",
                Pos::new(from, c).a1(),
                Pos::new(b.row, c).a1()
            ))
        } else if c == a.col {
            Cell::input("合計")
        } else {
            s.get(p).cloned().unwrap_or_default()
        };
        cell.fmt = fmt0;
        cell.fmt.bold = true;
        cell.fmt.borders.top = sheet::model::Edge::THIN;
        s.set(p, cell);
        n += 1;
    }
    n
}

/// データタブの「小計」(Excel の集計)。基準の列の値が変わる区切りごとに
/// 「〜 小計」の行(=SUM)を挿し、明細にグループ化(深さ1)を掛け、最後に
/// 総計の行を足す。**小計・総計の行はグループ化しない** — 詳細を畳んでも
/// 合計は見えたまま残る(発注者指摘 2026-08-04)。挿した式は最終の座標で
/// 書き、既存の式は insert_row が直す。返り値は区切りの数。
pub(crate) fn apply_subtotals(s: &mut sheet::Sheet, a: Pos, b: Pos, by: u32, vals: &[u32]) -> usize {
    // 区切り = 基準の列で連続する同じ値の並び(Excel と同じく、並べ替えは
    // 済んでいる前提。飛び飛びなら区切りもその数だけできる)
    let mut runs: Vec<(u32, u32, String)> = Vec::new();
    for r in a.row + 1..=b.row {
        let v = s.get(Pos::new(r, by)).map(|c| c.value.display()).unwrap_or_default();
        match runs.last_mut() {
            Some((_, end, label)) if *label == v => *end = r,
            _ => runs.push((r, r, v)),
        }
    }
    if runs.is_empty() {
        return 0;
    }
    // 枠を下から挿す(上の位置が狂わない): 総計の枠 → 各区切りの小計の枠
    s.insert_row(b.row + 1);
    for (_, end, _) in runs.iter().rev() {
        s.insert_row(end + 1);
    }
    // 中身は最終の座標で書く: k 番目の区切りの小計行 = end+1+k、
    // その明細は k 行ぶん下がっている。総計 = b.row+1+区切りの数
    let style = |s: &mut sheet::Sheet, p: Pos, text: &str| {
        let fmt0 = s.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
        let mut cell = Cell::input(text);
        cell.fmt = fmt0;
        cell.fmt.bold = true;
        cell.fmt.borders.top = sheet::model::Edge::THIN;
        s.set(p, cell);
    };
    let mut sub_rows = Vec::new();
    for (k, (start, end, label)) in runs.iter().enumerate() {
        let k = k as u32;
        let (det0, det1, srow) = (start + k, end + k, end + 1 + k);
        sub_rows.push(srow);
        style(s, Pos::new(srow, by), &ui::tf!("{} 小計", label));
        for c in vals {
            style(
                s,
                Pos::new(srow, *c),
                &format!("=SUM({}:{})", Pos::new(det0, *c).a1(), Pos::new(det1, *c).a1()),
            );
        }
        for r in det0..=det1 {
            s.row_outline.insert(r, 1);
        }
    }
    let trow = b.row + 1 + runs.len() as u32;
    style(s, Pos::new(trow, by), "総計");
    for c in vals {
        let refs: Vec<String> = sub_rows.iter().map(|r| Pos::new(*r, *c).a1()).collect();
        style(s, Pos::new(trow, *c), &format!("={}", refs.join("+")));
    }
    runs.len()
}

/// 控えたセルの**書式だけ**を写す(中身は残す)。帳票の枠の使い回し。
pub(crate) fn paste_formats(s: &mut sheet::Sheet, at: Pos, cells: &[Vec<Option<Cell>>]) -> usize {
    let mut n = 0usize;
    for (dr, row) in cells.iter().enumerate() {
        for (dc, src) in row.iter().enumerate() {
            let p = Pos::new(at.row + dr as u32, at.col + dc as u32);
            let fmt = src.as_ref().map(|c| c.fmt.clone()).unwrap_or_default();
            let mut cell = s.get(p).cloned().unwrap_or_default();
            cell.fmt = fmt;
            s.set(p, cell);
            n += 1;
        }
    }
    n
}

/// 格子を `at` から流し込む。返すのは置いたセルの数。
///
/// **書式は据え置く**(帳票の枠を壊さない — 範囲の Delete と同じ規則)。
/// `shift` があれば式の相対参照をずらす(このアプリの中でのコピー。
/// 外から来た TSV はずらさない — どこから切り取られたか知りようがない)。
pub(crate) fn paste_grid(
    s: &mut sheet::Sheet,
    at: Pos,
    grid: &[Vec<String>],
    shift: Option<(i64, i64)>,
) -> usize {
    let mut n = 0usize;
    for (dr, row) in grid.iter().enumerate() {
        for (dc, text) in row.iter().enumerate() {
            let p = Pos::new(at.row + dr as u32, at.col + dc as u32);
            let fmt = s.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
            let text = match (shift, text.starts_with('=')) {
                (Some((r, c)), true) => sheet::model::offset_refs(text, r, c),
                _ => text.clone(),
            };
            let mut cell = Cell::input(&text);
            cell.fmt = fmt;
            s.set(p, cell);
            n += 1;
        }
    }
    n
}

/// ゴールシークの解探索(割線法)。表の複製の上で var を動かし、
/// target が goal になる値を探す。見つからなければ None。
pub(crate) fn solve_goal(base: &sheet::Sheet, target: Pos, goal: f64, var: Pos) -> Option<f64> {
    let probe = |x: f64| -> f64 {
        let mut s = base.clone();
        let fmt = s.get(var).map(|c| c.fmt.clone()).unwrap_or_default();
        let mut cell = Cell::input(&format!("{x}"));
        cell.fmt = fmt;
        s.set(var, cell);
        recalc(&mut s);
        s.value(target).as_number() - goal
    };
    let x0 = base.get(var).map(|c| c.value.as_number()).unwrap_or(0.0);
    let (mut a, mut b) = (x0, if x0 == 0.0 { 1.0 } else { x0 * 1.1 });
    let (mut fa, mut fb) = (probe(a), probe(b));
    let tol = 1e-7 * goal.abs().max(1.0);
    for _ in 0..200 {
        if fb.abs() < tol {
            return Some(b);
        }
        if (fb - fa).abs() < f64::EPSILON {
            return None;
        }
        let c = b - fb * (b - a) / (fb - fa);
        if !c.is_finite() {
            return None;
        }
        (a, fa) = (b, fb);
        (b, fb) = (c, probe(c));
    }
    None
}

/// 画像の寸法(px)。PNG は IHDR、JPEG は SOF から(writer と同じ読み方)。
pub(crate) fn image_px(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let w = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
        return Some((w, h));
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2usize;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                return None;
            }
            let marker = bytes[i + 1];
            if marker == 0xFF || (0xD0..=0xD9).contains(&marker) || marker == 0x01 {
                i += 2;
                continue;
            }
            let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            if matches!(marker, 0xC0..=0xC3) {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
                return Some((w, h));
            }
            i += 2 + len;
        }
        return None;
    }
    None
}

/// 数値形式の一覧(本家のドロップダウン相当)。名前 → xlsx の書式コード。
/// None = 一般(書式なし)。会計・分数はまだ描けないので載せない(台帳に控え)
pub(crate) fn numfmts() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    vec![
        row(ui::item!("一般"), None),
        row(ui::item!("数値 (1234.56)"), Some("0.00")),
        row(ui::item!("桁区切り (1,234)"), Some("#,##0")),
        row(ui::item!("通貨 (¥1,234)"), Some("¥#,##0")),
        row(ui::item!("パーセント (12.34%)"), Some("0.00%")),
        row(ui::item!("指数 (1.23E+04)"), Some("0.00E+00")),
        row(ui::item!("短い日付 (2026/8/6)"), Some("yyyy/m/d")),
        row(ui::item!("長い日付 (2026年8月6日)"), Some("yyyy\"年\"m\"月\"d\"日\"")),
        row(ui::item!("時刻 (13:45:00)"), Some("h:mm:ss")),
        row(ui::item!("テキスト (@)"), Some("@")),
    ]
}

/// オートフィルタの控え。**隠す値**で持つ — 後から増えた値は隠れない
/// (Excel の「新しい項目は表示」に寄せた割り切り)。値は表示文字列。
/// 空セルは "" で持ち、画面では「(空白)」と見せる
pub(crate) struct AutoFilter {
    /// 張った範囲(左上=見出し行の左端, 右下)
    pub(crate) range: (Pos, Pos),
    /// 列(絶対の列番号)→ 隠す値。空の集合は持たない(=素通し)
    pub(crate) hide: std::collections::BTreeMap<u32, std::collections::BTreeSet<String>>,
}

/// 大文字小文字の変え方(本家の5択)。**鍵は日本語のまま** —
/// [`change_case`] の照合はこの鍵で行う(見出しだけが訳される)
pub(crate) fn case_modes() -> Vec<(&'static str, &'static str)> {
    vec![
        ui::item!("文の先頭だけ大文字"),
        ui::item!("すべて小文字"),
        ui::item!("すべて大文字"),
        ui::item!("単語の先頭を大文字"),
        ui::item!("大文字と小文字を入れ替え"),
    ]
}

/// 選んだ変え方で文字列を変換する(Unicode の upper/lower に従う)
pub(crate) fn change_case(t: &str, mode: &str) -> String {
    match mode {
        "すべて小文字" => t.to_lowercase(),
        "すべて大文字" => t.to_uppercase(),
        "文の先頭だけ大文字" => {
            let mut out = String::with_capacity(t.len());
            let mut done = false;
            for ch in t.to_lowercase().chars() {
                if !done && ch.is_alphabetic() {
                    out.extend(ch.to_uppercase());
                    done = true;
                } else {
                    out.push(ch);
                }
            }
            out
        }
        "単語の先頭を大文字" => {
            let mut out = String::with_capacity(t.len());
            let mut head = true;
            for ch in t.chars() {
                if ch.is_whitespace() {
                    head = true;
                    out.push(ch);
                } else if ch.is_alphabetic() {
                    if head {
                        out.extend(ch.to_uppercase());
                    } else {
                        out.extend(ch.to_lowercase());
                    }
                    head = false;
                } else {
                    // 数字や記号も語の中身(「3rd」の r は頭ではない)
                    out.push(ch);
                    head = false;
                }
            }
            out
        }
        "大文字と小文字を入れ替え" => t
            .chars()
            .flat_map(|ch| {
                if ch.is_uppercase() {
                    ch.to_lowercase().collect::<Vec<_>>()
                } else if ch.is_lowercase() {
                    ch.to_uppercase().collect()
                } else {
                    vec![ch]
                }
            })
            .collect(),
        _ => t.to_string(),
    }
}

/// セルのスタイル(本家の「セルのスタイル」。よく使う組だけ)。
/// 表オブジェクトは持たない方針どおり、掛けるのは普通の書式 —
/// どれも Ctrl+Z の1手で戻る
#[allow(clippy::type_complexity)]
/// フォントの色のパレット(本家の標準の色に寄せる。「自動」= 色なし)
/// 罫線の線種の一覧(本家のドロップダウンの12種)。名前 → BStyle
pub(crate) fn border_styles() -> Vec<(&'static str, &'static str, sheet::model::BStyle)> {
    use sheet::model::BStyle;
    vec![
        row(ui::item!("細い実線(既定)"), BStyle::Thin),
        row(ui::item!("極細"), BStyle::Hair),
        row(ui::item!("点線"), BStyle::Dotted),
        row(ui::item!("破線"), BStyle::Dashed),
        row(ui::item!("一点鎖線"), BStyle::DashDot),
        row(ui::item!("二点鎖線"), BStyle::DashDotDot),
        row(ui::item!("中太の実線"), BStyle::Medium),
        row(ui::item!("中太の破線"), BStyle::MediumDashed),
        row(ui::item!("中太の一点鎖線"), BStyle::MediumDashDot),
        row(ui::item!("中太の二点鎖線"), BStyle::MediumDashDotDot),
        row(ui::item!("太い実線"), BStyle::Thick),
        row(ui::item!("二重線"), BStyle::Double),
    ]
}

pub(crate) fn font_colors() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    vec![
        row(ui::item!("自動"), None),
        row(ui::item!("黒"), Some("1B1B1B")),
        row(ui::item!("赤"), Some("C00000")),
        row(ui::item!("橙"), Some("ED7D31")),
        row(ui::item!("黄"), Some("FFC000")),
        row(ui::item!("緑"), Some("70AD47")),
        row(ui::item!("青"), Some("4472C4")),
        row(ui::item!("紺"), Some("1F4E79")),
        row(ui::item!("紫"), Some("7030A0")),
        row(ui::item!("灰"), Some("7F7F7F")),
        row(ui::item!("白"), Some("FFFFFF")),
    ]
}

/// 塗りつぶしのパレット(帳票で使う薄い色を先に)
pub(crate) fn fill_colors() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    vec![
        row(ui::item!("色なし"), None),
        row(ui::item!("薄い黄"), Some("FFF2CC")),
        row(ui::item!("薄い青"), Some("DEEAF6")),
        row(ui::item!("薄い緑"), Some("E2EFDA")),
        row(ui::item!("薄い橙"), Some("FCE4D6")),
        row(ui::item!("薄い灰"), Some("D9D9D9")),
        row(ui::item!("黄"), Some("FFC000")),
        row(ui::item!("橙"), Some("ED7D31")),
        row(ui::item!("緑"), Some("70AD47")),
        row(ui::item!("青"), Some("4472C4")),
        row(ui::item!("灰"), Some("7F7F7F")),
    ]
}

pub(crate) fn cell_styles() -> Vec<(&'static str, &'static str, fn(&mut CellFormat))> {
    let f: Vec<(&'static str, &'static str, fn(&mut CellFormat))> = vec![
    row(ui::item!("標準"), |f| *f = CellFormat::default()),
    row(ui::item!("見出し"), |f| {
        f.bold = true;
        f.fill = Some("D5E8DC".into());
        f.borders.bottom = sheet::model::Edge::THIN;
    }),
    row(ui::item!("表題"), |f| {
        f.bold = true;
        f.size_c = Some(1600);
        f.color = Some("1B6E3C".into());
    }),
    row(ui::item!("良い"), |f| {
        f.fill = Some("C6EFCE".into());
        f.color = Some("006100".into());
    }),
    row(ui::item!("悪い"), |f| {
        f.fill = Some("FFC7CE".into());
        f.color = Some("9C0006".into());
    }),
    row(ui::item!("どちらでもない"), |f| {
        f.fill = Some("FFEB9C".into());
        f.color = Some("9C6500".into());
    }),
    row(ui::item!("メモ"), |f| {
        f.fill = Some("FFFFCC".into());
        f.borders = Borders::ALL;
    }),
    row(ui::item!("計算"), |f| {
        f.italic = true;
        f.fill = Some("F2F2F2".into());
        f.color = Some("7F7F7F".into());
    }),
    row(ui::item!("通貨"), |f| f.number_format = Some("¥#,##0".into())),
    row(ui::item!("パーセント"), |f| f.number_format = Some("0.0%".into())),
    ];
    f
}

pub(crate) fn col_name(c: u32) -> String {
    Pos::new(0, c).a1().trim_end_matches('1').to_string()
}

/// xlsx の paperSize → mm と名前。**B は JIS**(ECMA-376 の表は ISO だが、
/// 日本の事務様式と日本語版の印刷ドライバの実情は JIS。ここは日本のソフト)。
pub(crate) fn paper_mm(code: u32) -> Option<(f32, f32, &'static str)> {
    Some(match code {
        8 => (297.0, 420.0, "A3"),
        9 => (210.0, 297.0, "A4"),
        11 => (148.0, 210.0, "A5"),
        12 => (257.0, 364.0, "B4"),
        13 => (182.0, 257.0, "B5"),
        _ => return None,
    })
}

/// 条件付き書式の種類の、人に見せる名前(ルールの管理の一覧)。
pub(crate) fn cond_kind_name(k: &sheet::model::CondKind) -> String {
    use sheet::model::CondKind;
    match k {
        CondKind::Cmp(op, v) => format!("{} {}", ui::t!("値の比較"), format_args!("{op:?} {v}")),
        CondKind::Between(lo, hi, false) => ui::tf!("{} と {} の間", lo, hi).to_string(),
        CondKind::Between(lo, hi, true) => ui::tf!("{} と {} の外", lo, hi).to_string(),
        CondKind::Text(t) => ui::tf!("「{}」を含む", t).to_string(),
        CondKind::Dup(false) => ui::t!("重複する値").to_string(),
        CondKind::Dup(true) => ui::t!("一意の値").to_string(),
        CondKind::Top(n, false) => ui::tf!("上位 {}", n).to_string(),
        CondKind::Top(n, true) => ui::tf!("下位 {}", n).to_string(),
        CondKind::Avg(false) => ui::t!("平均より上").to_string(),
        CondKind::Avg(true) => ui::t!("平均より下").to_string(),
        CondKind::Bar(_) => ui::t!("データバー").to_string(),
        CondKind::Scale(..) => ui::t!("カラースケール").to_string(),
        CondKind::Icons(_) => ui::t!("アイコンセット").to_string(),
        // 式は左上を錨にした原文。一覧では `=` を付けて見せる(編集欄と同じ形)
        CondKind::Formula(f) => ui::tf!("数式 ={}", f).to_string(),
    }
}

/// いまの日時「YYYY-MM-DD HH:MM」(地方時)。
/// **中身は ui に置いた** — writer も同じ刻印を打つので、暦の算法を
/// 2箇所に持たない
pub(crate) fn now_stamp() -> String {
    ui::now_stamp()
}

/// フラッシュフィルの「作り方」の一片。
/// 元のセルの一部を切り出すか、そのまま置く字か。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Piece {
    /// 何本目の元の列の、何文字目から何文字
    Cut { col: usize, at: usize, len: usize },
    /// そのまま置く字(区切りの空白やハイフン)
    Lit(String),
}

/// **見本から作り方を推し量る。**
///
/// 本家のフラッシュフィルは中身を見て「たぶんこうだろう」を当てる。
/// こちらも同じ賭けをするが、**外したら黙って埋めない**ようにする:
/// 作り方は**見本を全部そのまま作り直せる**ものだけを採る。
///
/// 見つけ方は素朴で、答えの頭から順に「元のどれかの中にある一番長い並び」を
/// 取っていく。取れなければ1字ずつそのまま置く字にする。
pub(crate) fn flash_recipe(examples: &[(Vec<String>, String)]) -> Option<Vec<Piece>> {
    let (src, want) = examples.first()?;
    if want.is_empty() {
        return None;
    }
    let w: Vec<char> = want.chars().collect();
    let srcs: Vec<Vec<char>> = src.iter().map(|s| s.chars().collect()).collect();
    let mut out: Vec<Piece> = Vec::new();
    let mut i = 0usize;
    while i < w.len() {
        let mut best: Option<(usize, usize, usize)> = None; // (col, at, len)
        for (ci, s) in srcs.iter().enumerate() {
            if s.is_empty() {
                continue;
            }
            for at in 0..s.len() {
                let mut len = 0usize;
                while at + len < s.len() && i + len < w.len() && s[at + len] == w[i + len] {
                    len += 1;
                }
                if len >= 1 && best.map(|(_, _, bl)| len > bl).unwrap_or(true) {
                    best = Some((ci, at, len));
                }
            }
        }
        match best {
            Some((col, at, len)) if len >= 1 => {
                out.push(Piece::Cut { col, at, len });
                i += len;
            }
            _ => {
                // そのまま置く字。続くぶんはまとめる
                match out.last_mut() {
                    Some(Piece::Lit(s)) => s.push(w[i]),
                    _ => out.push(Piece::Lit(w[i].to_string())),
                }
                i += 1;
            }
        }
    }
    // **見本を全部作り直せること。** 1つでも合わなければ諦める
    for (s, want) in examples {
        if flash_apply(&out, s)? != *want {
            return None;
        }
    }
    Some(out)
}

/// 作り方を元のセルに当てる。切り出しがはみ出したら諦める(None)
pub(crate) fn flash_apply(recipe: &[Piece], src: &[String]) -> Option<String> {
    let mut out = String::new();
    for p in recipe {
        match p {
            Piece::Lit(s) => out.push_str(s),
            Piece::Cut { col, at, len } => {
                let s: Vec<char> = src.get(*col)?.chars().collect();
                if at + len > s.len() {
                    return None;
                }
                out.extend(&s[*at..at + len]);
            }
        }
    }
    Some(out)
}

/// **参照の `$` を回す**(F4)。`A1 → $A$1 → A$1 → $A1 → A1` の順。
///
/// `cur`(バイト位置)の**直前にある参照**を1つ選んで回し、
/// (直した式, 新しいカーソル位置)を返す。参照が見つからなければ None。
///
/// 本家は選択中の参照を回すが、こちらは打っている途中を想定して
/// 「カーソルの手前」を見る。**行だけ・列だけの $ も一巡に入れる** —
/// 表を横に引き写すときに列だけ止めたい、が実際によくある。
pub(crate) fn cycle_ref_at(text: &str, cur: usize) -> Option<(String, usize)> {
    let b = text.as_bytes();
    let cur = cur.min(b.len());
    // カーソルの手前をさかのぼって「$?英字+$?数字」の並びを探す
    let is_col = |c: u8| c.is_ascii_alphabetic();
    let is_dig = |c: u8| c.is_ascii_digit();
    let mut end = cur;
    // カーソルが参照の途中にいるなら、その参照の終わりまで進める
    while end < b.len() && is_dig(b[end]) {
        end += 1;
    }
    let mut i = end;
    // 数字
    while i > 0 && is_dig(b[i - 1]) {
        i -= 1;
    }
    if i == end {
        return None; // 数字が無い = 参照ではない
    }
    if i > 0 && b[i - 1] == b'$' {
        i -= 1;
    }
    let row_start = i;
    // 英字
    let mut j = i;
    while j > 0 && is_col(b[j - 1]) {
        j -= 1;
    }
    if j == row_start {
        return None; // 英字が無い
    }
    if j > 0 && b[j - 1] == b'$' {
        j -= 1;
    }
    let start = j;
    // 直前が英数字なら関数名などの一部 — 参照ではない
    if start > 0 && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_') {
        return None;
    }
    let refs = &text[start..end];
    let plain: String = refs.chars().filter(|c| *c != '$').collect();
    let (col, row): (String, String) = (
        plain.chars().take_while(|c| c.is_ascii_alphabetic()).collect(),
        plain.chars().skip_while(|c| c.is_ascii_alphabetic()).collect(),
    );
    // 列は3文字まで(XFD)。それ以上は参照ではない
    if col.len() > 3 {
        return None;
    }
    // 直後が `(` なら関数名(LOG10( など)。$ を付けたら壊れる
    if b.get(end) == Some(&b'(') {
        return None;
    }
    let had_col = refs.starts_with('$');
    let had_row = refs.trim_start_matches('$').contains('$');
    let next = match (had_col, had_row) {
        (false, false) => format!("${col}${row}"),
        (true, true) => format!("{col}${row}"),
        (false, true) => format!("${col}{row}"),
        (true, false) => format!("{col}{row}"),
    };
    let mut out = String::with_capacity(text.len() + 2);
    out.push_str(&text[..start]);
    out.push_str(&next);
    out.push_str(&text[end..]);
    Some((out, start + next.len()))
}

/// スライサー(列の値をボタンで並べ、押して絞る)。**見え方だけ** —
/// 絞り込みと同じで、保存される中身は変わらない。
///
/// 前は `(u32, BTreeSet<String>, bool)` の組だった。並び順と
/// 「空の項目を隠す」が加わって組では読めなくなったので名前を付けた
/// (2026-08-10)。
pub(crate) struct Slicer {
    /// 見ている列
    pub(crate) col: u32,
    /// 選んだ値(空 = 素通し)
    pub(crate) sel: std::collections::BTreeSet<String>,
    /// 複数選択か
    pub(crate) multi: bool,
    /// 降順に並べるか(既定は昇順)
    pub(crate) desc: bool,
    /// **他の絞りで一行も残っていない値を並べないか。**
    /// 既定は並べる — 押せるのに何も起きないボタンより、
    /// 「その値の行は今は無い」が見えるほうが分かる場面もある
    pub(crate) hide_empty: bool,
}

/// スライサーの値の並べ方。**数だけの値は数として比べる** —
/// 文字として比べると 10 が 2 より前に来て、伝票番号の列が読めなくなる。
///
/// 漢字の五十音順はしない(読みが要る)。文字は符号位置の順で、
/// かなは五十音、英字は ABC 順になる — そこまでが正直にできる範囲
pub(crate) fn slicer_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => a.cmp(b),
    }
}

/// スライサーに並べる値。
///
/// `rows` は見出しの下の各行の (その行のその列の値, いま他の絞りで見えているか)。
/// **「いま見えているか」に自分の選びは入れない** — 入れると選んだ瞬間に
/// 他のボタンが消えて、選び直せなくなる。
///
/// 空欄は値ではないので「(空白)」として**並べ替えの外・いちばん最後**に置く。
///
/// 多すぎる列で画面が埋まらないよう 64 個まで。返す2つ目は**そこで落とした
/// 数**。呼ぶ側はこれを画面に出すこと — 黙って切ると、押しても絞れない
/// 理由が分からない。⊘ で外したぶんはここに数えない(外したのは承知の上)
pub(crate) fn slicer_items(
    rows: &[(String, bool)],
    desc: bool,
    hide_empty: bool,
) -> (Vec<String>, usize) {
    let mut seen: std::collections::HashMap<&str, bool> = Default::default();
    let mut has_blank = false;
    let mut blank_live = false;
    for (v, live) in rows {
        if v.is_empty() {
            has_blank = true;
            blank_live |= *live;
        } else {
            let e = seen.entry(v.as_str()).or_insert(false);
            *e |= *live;
        }
    }
    let mut items: Vec<&str> = seen
        .iter()
        .filter(|(_, live)| !hide_empty || **live)
        .map(|(v, _)| *v)
        .collect();
    items.sort_by(|a, b| slicer_cmp(a, b));
    if desc {
        items.reverse();
    }
    let cut = items.len().saturating_sub(64);
    let mut out: Vec<String> = items.into_iter().take(64).map(|s| s.to_string()).collect();
    if has_blank && (!hide_empty || blank_live) {
        out.push(ui::t!("(空白)").to_string());
    }
    (out, cut)
}
