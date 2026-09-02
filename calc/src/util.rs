//! main.rs からの純移動(2026-08-06 の分割)。挙動は変えない。

use crate::*;

pub(crate) use ops::{font_data, image_px};

pub(crate) const ROW_H: f32 = 24.0;

/// 表の端(この番号まで。0 から数える)。**行 10000・列 256**。
/// 列・行の丸ごと選択と、カーソルの動ける限界がこれを見る。
/// 前は移動の側に 9999 / 255 を直書きしていた(2026-08-14 に定数へ)
pub(crate) const LAST_ROW: u32 = 9999;
pub(crate) const LAST_COL: u32 = 255;
/// `RRGGBB` を色にする。読めなければ黒
/// 下地に選択の緑を混ぜる。**塗りを置き換えない** — 選択中も帳票本来の色が
/// 透けて見える(選択を解かないと色が確かめられない、を避ける)。
/// セルのスタイル1つ(鍵, 見出し, 掛ける手)。
type CellStyle = (&'static str, &'static str, fn(&mut CellFormat));

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
    // 成分の読みは ops(writer と同じ)。gpui に包むのはここだけ
    gpui::Rgba { r: ops::hex(s, 0), g: ops::hex(s, 1), b: ops::hex(s, 2), a: 1.0 }
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
/// ```text
/// self.pick = Some((menu(&[ui::item!(…), ui::item!(…)]), at));
/// ```
///
/// 字下げの塊は Rust の見本として組まれるので、`text` と名乗ります
/// (2026-08-19 に calc をライブラリへ切り出したら doc-test が動き出し、
/// 説明用の `…` が読めずに落ちました)。
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
/// 字の大きさ(pt)を画面の px にする。**96dpi 固定で 96/72**。
///
/// **画面の dpi は機械の実 dpi を読まない**(発注者確定 2026-08-14)。
/// 紙は A4=210mm と物理で決まり、PDF は 72dpi の pt で出る。画面だけ
/// 機械ごとの実 dpi(この機械は実 162.6dpi)で描くと、同じブックが
/// 機械ごとに違う紙面になり、印刷と合わせられない。96 固定なら
/// どの機械でも同じ紙面。実寸で見たいときは Ctrl+ホイールで拡大する。
///
/// **GNOME の文字倍率(text-scaling-factor)には追随しない**(発注者確定
/// 2026-08-14)。あれは字だけを大きくする設定で、箱(セルの幅・行の高さ・
/// ボタン)は元のまま — 字がはみ出して切れ、画面と紙も食い違う。
/// 画面を大きくする道は Ctrl+= (ui_scale) と Ctrl+ホイール(zoom)で、
/// どちらも箱ごと動く。詳しくは docs/sekkei/ui.ja.md の「画面の大きさ」
///
/// 前は 24/15×0.8 = 1.28 倍で、正しい 1.333… より **4%小さかった**。
/// おまけに書式の無いセルの既定は 12.5px の直書きで、11pt を 1.136 倍
/// した値 — 二重にずれていた(発注者 2026-08-14「フォントの大きさが
/// 小さすぎる」)。換算はこの1本に集める(5箇所に散っていた)。
/// `size_c` はセルの持つ 1/100 pt。無ければ既定の 11pt
pub(crate) fn cell_font_px(size_c: Option<u32>, zoom: f32) -> f32 {
    let pt = size_c.map(|c| c as f32 / 100.0).unwrap_or(11.0);
    zoom * pt * 96.0 / 72.0
}

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
        row(ui::item!("common_business_forms"), "〒℡№㈱㈲〆※‰°′″"),
        row(ui::item!("marks"), "○●◎△▲▽▼□■◇◆☆★×✓☑☐"),
        row(ui::item!("arrows"), "→←↑↓⇒⇐⇔↔↗↘↙↖"),
        row(ui::item!("circled_numbers"), "①②③④⑤⑥⑦⑧⑨⑩⑪⑫"),
        row(ui::item!("currency"), "¥＄€£¢₩₹"),
        row(ui::item!("maths"), "±×÷≠≒≦≧∞√∑∫"),
        row(ui::item!("brackets_separators"), "「」『』【】〔〕・…‥※〜"),
        row(ui::item!("greek"), "αβγδεζηθλμπστφω"),
    ]
}

// **一覧の位置決めは face::combo に移しました**(2026-08-20。SEKKEI
// 「リボンのドロップダウンを1つの仕組みにする」の手順1)。writer にも
// 同じ形で効かせるためで、gpui を持たない層に置いてあります
pub(crate) use face::combo::{pop_at_click, pop_place, pop_under, POP_MIN_W, POP_W};
// `pop_x` は `pop_under` の中だけで使うので、こちらは試験のためだけに借ります
#[cfg(test)]
pub(crate) use face::combo::pop_x;

/// 描く行の並び。上の帯を頭に、残りは窓から。
///
/// `top` は帯の (行数, 先頭行) です。固定の帯は先頭行が 0 で、分割の帯は
/// 動きます。帯が無いときは `None`。
///
/// 固定は同じ行を二度出しません。分割は出します — **同じ場所の上と下を
/// 見比べる**のが分割の使い道なので、重なりを止めると用を成しません。
pub(crate) fn grid_rows(top: Option<(u32, u32)>, view: Pos, n: u32) -> Vec<u32> {
    let (f, s) = top.unwrap_or((0, 0));
    let f = f.min(n);
    let mut out: Vec<u32> = (s..s + f).collect();
    let start = if s == 0 { view.row.max(f) } else { view.row };
    while (out.len() as u32) < n {
        let next = start + out.len() as u32 - f;
        out.push(next);
    }
    out
}

/// 描く列の並び。grid_rows と同じ役割です。
pub(crate) fn grid_cols(top: Option<(u32, u32)>, view: Pos, n: u32) -> Vec<u32> {
    let (f, s) = top.unwrap_or((0, 0));
    let f = f.min(n);
    let mut out: Vec<u32> = (s..s + f).collect();
    let start = if s == 0 { view.col.max(f) } else { view.col };
    while (out.len() as u32) < n {
        let next = start + out.len() as u32 - f;
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
/// 好きな頭で始まる、まだ無いシート名。「予測」「予測2」…
pub(crate) fn unique_sheet_name_for(book: &Book, head: &str) -> String {
    if !book.sheets.iter().any(|s| s.name == head) {
        return head.to_string();
    }
    let mut n = 2;
    loop {
        let name = format!("{head}{n}");
        if !book.sheets.iter().any(|s| s.name == name) {
            return name;
        }
        n += 1;
    }
}

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

// 改名の参照書き換え(rename_refs_in / rename_sheet_refs)は 2026-08-12 に
// kumihan::book::refs へ**純移動**した — Python(pysheet)の改名でも式が
// 追随するように。呼び側(picks.rs・tests.rs)の名前が変わらないよう再輸出する
#[allow(unused_imports)] // rename_refs_in は tests.rs だけが使う
pub(crate) use book::{rename_refs_in, rename_sheet_refs};

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
pub(crate) fn range_tsv(s: &book::Sheet, a: Pos, b: Pos) -> String {
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
pub(crate) fn paste_values_cells(s: &mut book::Sheet, at: Pos, cells: &[Vec<Option<Cell>>]) -> usize {
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
pub(crate) fn paste_values_text(s: &mut book::Sheet, at: Pos, grid: &[Vec<String>]) -> usize {
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
    /// 分類の引き出しが開いているか(本家と同じ形の選び方)
    pub(crate) open: bool,
}

/// 分類のタブ。「すべて」+ funcs.rs の分類。**日本語のまま持つ** —
/// これは絞り込みの照合に使う鍵で、画面に出すときだけ訳す
pub(crate) const FN_GROUPS: &[&str] = &["all", "math_trig", "statistics", "text_functions", "logical", "date_time", "lookup_reference", "financial", "information", "engineering", "database"];

/// 分類のタブに出す語。**変数を `ui::tr` に渡さず、1つずつ literal で書く。**
///
/// 前は `ui::tr(g)` と鍵を変数で渡していた。動きはするが、**文言の門番は
/// 印の付いた literal しか見られない**ので、訳が無くても誰も気づかない。
/// 実際、9 個のうち5個(すべて・数学・文字列・論理・財務)が未訳のまま
/// 画面に出ていた(2026-08-11、関数の説明を各言語に繋いだときに見つけた)。
pub(crate) fn fn_group_label(key: &'static str) -> &'static str {
    match key {
        "all" => ui::t!("all"),
        // **名前はリボンの族のボタンに揃える。** 同じ関数の集まりなのに
        // タブが「数学」でボタンが「数学/三角」では、同じ製品の中で
        // 2つの名前を持つことになる。はじめ短い名前を別に作ってしまい、
        // 訳も別に頼んでいた(2026-08-11、下請けの指摘で気づいた)。
        // 一語の鍵は他の意味と衝突もする — 「日付」は数値の書式の
        // 見出し、「検索」は検索コマンド(英語は "find")
        "math_trig" => ui::t!("math_trig"),
        "statistics" => ui::t!("statistics"),
        "text_functions" => ui::t!("text_functions"),
        "logical" => ui::t!("logical"),
        "date_time" => ui::t!("date_time"),
        "lookup_reference" => ui::t!("lookup_reference"),
        "financial" => ui::t!("financial"),
        "information" => ui::t!("information"),
        "engineering" => ui::t!("engineering"),
        "database" => ui::t!("database"),
        other => other,
    }
}

/// 分類のタブから「その族の一覧」を開くときのコマンド id。
///
/// **綴りは `FN_GROUPS` が正。** 前はこの照合が picks.rs に直に書いてあり、
/// 綴りがずれると黙って既定の `fn-lookup` に落ちて**別の一覧が出る**
/// (2026-08-11、「日付」を「日付・時刻」に広げたときに踏みかけた)。
/// 1箇所に集めて、試験が `FN_GROUPS` と突き合わせられるようにした。
pub(crate) fn fn_group_cmd(group: &str) -> &'static str {
    match group {
        "statistics" | "math_trig" => "fn-math",
        "financial" => "fn-financial",
        "date_time" => "fn-datetime",
        "text_functions" => "fn-text",
        "logical" => "fn-logical",
        // 検索/行列 と 情報 はここ。**既定に落ちるのが正しい2つ**
        _ => "fn-lookup",
    }
}

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

/// ソルバーの制約の記号。**後ろの2つは右辺を取りません**
/// (2026-08-21 の D群「ソルバーの整数・バイナリ制約」)。
///
/// Excel と同じ形で、`$B$2:$B$4` `整数` のように左辺だけを書きます。
/// バイナリは 0 か 1 — 整数に 0〜1 の枠を足した物です。
///
/// *記号は訳しません。* `<=` などと同じ欄に並ぶ印で、画面では
/// `SOLVER_OP_LABELS` の語を出します。
pub(crate) const SOLVER_OPS: [&str; 5] = ["<=", "=", ">=", "int", "bin"];

/// 記号を画面に出すときの語(`int` と `bin` だけ言葉にする)
pub(crate) fn solver_op_label(i: usize) -> String {
    match i {
        3 => ui::t!("whole_number").to_string(),
        4 => ui::t!("binary").to_string(),
        _ => SOLVER_OPS[i.min(2)].to_string(),
    }
}

/// 右辺が要る記号か(整数・バイナリは要らない)
pub(crate) fn solver_op_needs_rhs(i: usize) -> bool {
    i < 3
}

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
    pub(crate) keep: Option<book::Validation>,
    /// 開いたときの既存の規則(「同じ設定の他のセル」の比較の相手)
    pub(crate) was: Option<book::Validation>,
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
        ui::t!("any_value"),
        ui::t!("whole_number"),
        ui::t!("decimal"),
        ui::t!("list"),
        ui::t!("text_length"),
    ]
}
/// kind の添字 → xlsx の type
pub(crate) const DV_KIND_XLSX: [&str; 5] = ["", "whole", "decimal", "list", "textLength"];
/// データ(比較)の一覧 `(xlsx の operator, 見出し)`。並びは xlsx の operator と対。
/// **引き当ては operator**(訳さない字)、画面は見出し
pub(crate) fn dv_ops() -> [(&'static str, &'static str); 8] {
    [
        // **左は xlsx の operator。** 訳さない字なので記号にしません
        ("between", ui::t!("between_2")),
        ("notBetween", ui::t!("not_between")),
        ("equal", ui::t!("equal")),
        ("notEqual", ui::t!("not_equal")),
        ("greaterThan", ui::t!("greater_than")),
        ("lessThan", ui::t!("less_than")),
        ("greaterThanOrEqual", ui::t!("greater_than_equal")),
        ("lessThanOrEqual", ui::t!("less_than_equal")),
    ]
}
/// エラー警告のスタイル `(xlsx の errorStyle, 見出し)`。引き当ては errorStyle
pub(crate) fn dv_styles() -> [(&'static str, &'static str); 3] {
    [
        ("stop", ui::t!("stop")),
        ("warning", ui::t!("warning")),
        ("information", ui::t!("information")),
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
        row(ui::item!("list"), vec![
            row(ui::item!("card_list"), "block-list"),
            row(ui::item!("vertical_list"), "vbox-list"),
            row(ui::item!("pyramid_list"), "pyramid-list"),
        ]),
        row(ui::item!("process"), vec![
            row(ui::item!("basic_steps"), "basic-process"),
            row(ui::item!("process"), "chevron-process"),
            row(ui::item!("timeline"), "timeline"),
        ]),
        row(ui::item!("cycle"), vec![
            row(ui::item!("basic_cycle"), "basic-cycle"),
            row(ui::item!("block_cycle"), "block-cycle"),
        ]),
        row(ui::item!("hierarchy"), vec![
            row(ui::item!("organisation_chart"), "org-chart"),
            row(ui::item!("hierarchy"), "hierarchy"),
        ]),
        row(ui::item!("relationship"), vec![row(ui::item!("basic_venn"), "venn")]),
        row(ui::item!("matrix"), vec![row(ui::item!("basic_matrix"), "matrix")]),
        row(ui::item!("pyramid"), vec![row(ui::item!("basic_pyramid"), "pyramid")]),
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
        ui::item!("sum"),
        ui::item!("average"),
        ui::item!("count"),
        ui::item!("maximum"),
        ui::item!("minimum"),
    ]
}


/// **日付の粒でまとめたときの札**(タイムライン。2026-08-22 の D群)。
///
/// `serial` は通し番号(セルが数のとき)、`text` は画面に出ている字です。
/// どちらからも日付が読めなければ `None` を返します。**日付でない行を
/// 勝手にどこかの束へ入れません** — 入れると、無関係な行が一緒に消えます。
///
/// 札の形はピボットの日付のグループ化(`PIVOT_PY`)と揃えてあります。
/// 揃っていないと、同じ月を指しているのに別の字になります。
pub(crate) fn date_bucket(
    serial: Option<f64>,
    text: &str,
    grain: &str,
    date1904: bool,
) -> Option<String> {
    if grain.is_empty() {
        return None;
    }
    let (y, m, _d) = match serial {
        // 通し番号。1 未満は時刻だけの値なので日付として扱わない
        Some(n) if n >= 1.0 => {
            let ep = book::calc::excel_epoch(date1904);
            let (y, m, d) = book::calc::civil_from_days(n.floor() as i64 - ep);
            (y, m, d)
        }
        _ => {
            let t = text.trim();
            let sep = if t.contains('-') { '-' } else { '/' };
            let mut it = t.split(sep);
            let y: i64 = it.next()?.trim().parse().ok()?;
            let m: i64 = it.next()?.trim().parse().ok()?;
            let d: i64 = it.next().unwrap_or("1").trim().parse().unwrap_or(1);
            if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
                return None;
            }
            (y, m, d)
        }
    };
    Some(match grain {
        "years" => format!("{y}年"),
        "quarters" => format!("{}年Q{}", y, (m + 2) / 3),
        // 月は `2026-08`。ピボットの %Y-%m と同じ
        _ => format!("{y}-{m:02}"),
    })
}

/// 日付の粒の選択肢。**空(値そのもの)はここに入れません** —
/// 鍵と見出しが違う組は `ui::item!` に載らないので、呼ぶ側で別に書きます
pub(crate) fn slicer_grains() -> Vec<(&'static str, &'static str)> {
    vec![ui::item!("months"), ui::item!("quarters"), ui::item!("years")]
}

/// 粒の見出し。空なら「値そのもの」
pub(crate) fn slicer_grain_label(grain: &str) -> String {
    if grain.is_empty() {
        return ui::t!("values_themselves").to_string();
    }
    slicer_grains()
        .iter()
        .find(|(k, _)| *k == grain)
        .map(|(_, l)| ui::tr(l).to_string())
        .unwrap_or_else(|| grain.to_string())
}

/// おすすめのピボットの1つ。
///
/// **こちらから作りはしません。** 候補を並べて、人が選んで、人が押します
/// (2026-08-09 発注者確定の方針)。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PivotSuggest {
    pub rows_sel: Vec<String>,
    pub cols_sel: Vec<String>,
    pub value: String,
    pub agg: &'static str,
}

/// 列の中身が数として読めるか。**6割を超えたら数の列**とみなします。
/// 全部でなくてよいのは、実物の表には「-」や「未定」が混ざるためです
fn num_cols(vals: &[String]) -> bool {
    let content: Vec<&String> = vals.iter().filter(|v| !v.trim().is_empty()).collect();
    if content.is_empty() {
        return false;
    }
    let numbers = content
        .iter()
        .filter(|v| v.replace([',', ' ', '\u{a0}'], "").parse::<f64>().is_ok())
        .count();
    numbers * 10 >= content.len() * 6
}

/// 空でない値の種類の数。
fn n_kinds(vals: &[String]) -> usize {
    let mut v: Vec<&str> = vals.iter().map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
    v.sort_unstable();
    v.dedup();
    v.len()
}

/// **おすすめのピボットの形**(2026-08-21 の D群)。
///
/// `cols[i]` は `headers[i]` の列の中身(見出しの行は含めません)です。
/// 決め方は次のとおりで、乱数も学習も使いません。同じ表からは毎回同じ
/// 候補が出ます。
///
/// * 数の列 → 値の候補
/// * 数でなく、種類が2つ以上で、行数の半分以下(または12以下)の列 → 行の候補。
///   1行に1つしかない列(伝票番号や氏名)は、まとめても意味がないので外します
/// * 行の候補のうち種類が8つ以下のもの → 列の候補
///
/// 候補は種類の少ない順・左の列からの順で、6つまで返します。
pub(crate) fn pivot_suggestions(headers: &[String], cols: &[Vec<String>]) -> Vec<PivotSuggest> {
    let n = cols.first().map(|c| c.len()).unwrap_or(0);
    if n == 0 || headers.len() != cols.len() {
        return Vec::new();
    }
    let cap = (n / 2).max(12);
    let mut line_cands: Vec<(usize, usize)> = Vec::new(); // (種類, 列)
    let mut value_cands: Vec<usize> = Vec::new();
    for (i, c) in cols.iter().enumerate() {
        if headers[i].trim().is_empty() {
            continue;
        }
        if num_cols(c) {
            value_cands.push(i);
        } else {
            let k = n_kinds(c);
            if (2..=cap).contains(&k) {
                line_cands.push((k, i));
            }
        }
    }
    line_cands.sort_unstable();
    let mut out: Vec<PivotSuggest> = Vec::new();
    let push = |s: PivotSuggest, out: &mut Vec<PivotSuggest>| {
        if !out.contains(&s) && out.len() < 6 {
            out.push(s);
        }
    };
    for &(_, r) in line_cands.iter().take(2) {
        for &v in value_cands.iter().take(2) {
            push(
                PivotSuggest {
                    rows_sel: vec![headers[r].clone()],
                    cols_sel: Vec::new(),
                    value: headers[v].clone(),
                    agg: "sum",
                },
                &mut out,
            );
        }
    }
    // 2つの見出しで縦横に広げる形。列は種類の少ないものだけ
    if let (Some(&(_, r)), Some(&v)) = (line_cands.first(), value_cands.first()) {
        if let Some(&(_, c)) = line_cands.iter().find(|&&(k, i)| i != r && k <= 8) {
            push(
                PivotSuggest {
                    rows_sel: vec![headers[r].clone()],
                    cols_sel: vec![headers[c].clone()],
                    value: headers[v].clone(),
                    agg: "sum",
                },
                &mut out,
            );
        }
    }
    // 数の列が1つも無くても、件数なら数えられます
    if let Some(&(_, r)) = line_cands.first() {
        push(
            PivotSuggest {
                rows_sel: vec![headers[r].clone()],
                cols_sel: Vec::new(),
                value: headers[r].clone(),
                agg: "count",
            },
            &mut out,
        );
    }
    out
}

/// おすすめの1つを、人が読める1行にします。
pub(crate) fn pivot_suggest_label(s: &PivotSuggest) -> String {
    let agg = pivot_aggs()
        .iter()
        .find(|(k, _)| *k == s.agg)
        .map(|(_, l)| (*l).to_string())
        .unwrap_or_else(|| s.agg.to_string());
    match s.cols_sel.first() {
        Some(c) => ui::tf!(
            "rows_columns",
            s.rows_sel.join("・"),
            c.clone(),
            s.value.clone(),
            agg
        )
        .to_string(),
        None => ui::tf!("rows", s.rows_sel.join("・"), s.value.clone(), agg)
            .to_string(),
    }
}



/// 連続した数の判定(フィルハンドルとリボンの「フィル」が共有する)。
///
/// 全部が式でない数で、1つなら 1 ずつ増やす、2つ以上なら間隔が一定の
/// ときだけ `Some((最後の値, 間隔))`。文字や式が混じれば None(写す側へ)
pub(crate) fn number_series(vals: &[Option<Cell>]) -> Option<(f64, f64)> {
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
        1 => Some((nums[0], 1.0)),
        _ => {
            let step = nums[1] - nums[0];
            let ok = nums.windows(2).all(|w| (w[1] - w[0] - step).abs() < 1e-9);
            ok.then(|| (*nums.last().unwrap(), step))
        }
    }
}

/// 小計の集計方法(一覧のキー → 式に書く関数名)。並びは一覧の並び
pub(crate) const SUBTOTAL_FUNCS: &[(&str, &str)] = &[
    ("sum", "SUM"),
    ("average", "AVERAGE"),
    ("count", "COUNT"),
    ("maximum", "MAX"),
    ("minimum", "MIN"),
];

/// データタブの「小計」(Excel の集計)。基準の列の値が変わる区切りごとに
/// 「〜 小計」の行(=SUM)を挿し、明細にグループ化(深さ1)を掛け、最後に
/// 総計の行を足す。**小計・総計の行はグループ化しない** — 詳細を畳んでも
/// 合計は見えたまま残る(発注者指摘 2026-08-04)。挿した式は最終の座標で
/// 書き、既存の式は insert_row が直す。返り値は区切りの数。
/// `func` は SUM / AVERAGE / COUNT / MAX / MIN のどれか(画面の
/// 「集計方法」の一覧から来る)
pub(crate) fn apply_subtotals_with(
    s: &mut book::Sheet,
    a: Pos,
    b: Pos,
    by: u32,
    vals: &[u32],
    func: &str,
) -> usize {
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
    let style = |s: &mut book::Sheet, p: Pos, text: &str| {
        let fmt0 = s.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
        let mut cell = Cell::input(text);
        cell.fmt = fmt0;
        cell.fmt.bold = true;
        cell.fmt.borders.top = book::Edge::THIN;
        s.set(p, cell);
    };
    let mut sub_rows = Vec::new();
    let mut details: Vec<(u32, u32)> = Vec::new();
    for (k, (start, end, label)) in runs.iter().enumerate() {
        let k = k as u32;
        let (det0, det1, srow) = (start + k, end + k, end + 1 + k);
        sub_rows.push(srow);
        details.push((det0, det1));
        style(s, Pos::new(srow, by), &ui::tf!("subtotal", label));
        for c in vals {
            style(
                s,
                Pos::new(srow, *c),
                &format!("={func}({}:{})", Pos::new(det0, *c).a1(), Pos::new(det1, *c).a1()),
            );
        }
        for r in det0..=det1 {
            s.row_outline.insert(r, 1);
        }
    }
    let trow = b.row + 1 + runs.len() as u32;
    // **文書に書き込む字は訳を通します。** 鍵をそのまま書くと、
    // 日本語の人の表に英語の見出しが入ります(2026-08-26)
    style(s, Pos::new(trow, by), ui::t!("grand_totals"));
    for c in vals {
        // 合計は小計の行を足す。平均・個数・最大・最小は小計を足しても
        // 答えにならないので、明細の範囲を並べて同じ関数に掛ける
        let text = if func == "SUM" {
            let refs: Vec<String> = sub_rows.iter().map(|r| Pos::new(*r, *c).a1()).collect();
            format!("={}", refs.join("+"))
        } else {
            let refs: Vec<String> = details
                .iter()
                .map(|(d0, d1)| format!("{}:{}", Pos::new(*d0, *c).a1(), Pos::new(*d1, *c).a1()))
                .collect();
            format!("={func}({})", refs.join(","))
        };
        style(s, Pos::new(trow, *c), &text);
    }
    runs.len()
}

/// 控えたセルの**書式だけ**を写す(中身は残す)。帳票の枠の使い回し。
pub(crate) fn paste_formats(s: &mut book::Sheet, at: Pos, cells: &[Vec<Option<Cell>>]) -> usize {
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

/// **すべて**(中身と書式)を貼る — 普通の Ctrl+V。このアプリでコピーした
/// 範囲だけが通る道で、控えたセルをそのまま置く。式の相対参照は `shift`。
///
/// 本家の普通の貼り付けは書式も運ぶ(発注者 2026-08-14)。値だけ・書式だけ・
/// 式だけは「形式を選択して貼り付け」の側にある — 分かれ道はそちら
pub(crate) fn paste_all_cells(
    s: &mut book::Sheet,
    at: Pos,
    cells: &[Vec<Option<Cell>>],
    shift: Option<(i64, i64)>,
) -> usize {
    let mut n = 0usize;
    for (dr, row) in cells.iter().enumerate() {
        for (dc, src) in row.iter().enumerate() {
            let p = Pos::new(at.row + dr as u32, at.col + dc as u32);
            match src {
                Some(src) => {
                    let mut cell = src.clone();
                    if let (Some((r, c)), Some(f)) = (shift, src.formula.as_deref()) {
                        cell.formula = Some(book::offset_refs(f, r, c));
                    }
                    s.set(p, cell);
                }
                // 空のセルを貼るのも「貼る」— 元が空なら先も空にする
                // (書式は元のまま。中身だけ消す — 帳票の枠を壊さない)
                None => {
                    let fmt = s.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                    s.set(p, Cell { formula: None, value: book::Value::Empty, fmt });
                }
            }
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
    s: &mut book::Sheet,
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
                (Some((r, c)), true) => book::offset_refs(text, r, c),
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
pub(crate) fn solve_goal(base: &book::Sheet, target: Pos, goal: f64, var: Pos) -> Option<f64> {
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

/// 数値形式の一覧(本家のドロップダウン相当)。名前 → xlsx の書式コード。
/// None = 一般(書式なし)。会計・分数はまだ描けないので載せない(台帳に控え)
/// 選べる通貨。**(鍵, 記号, 小数の桁)**。
///
/// **通貨は読む人の言語ではなく、その帳票のお金**(2026-08-10 発注者確定)。
/// だから言語から引かず、人に選ばせる。日本の会社が €建ての請求書を作るのは
/// 普通で、相手が独語圏とは限らない。言語から決めると、**円の帳票を
/// ドイツ語で開いた人に € と見せる**ことになる — 見た目ではなく
/// 金額の意味を書き換えて見せることなので、日付のずれより重い。
///
/// 並びは「帳票でよく使う順」(記号の一覧と同じ考え方)。円が既定。
/// **小数の桁も通貨で決まる** — 「¥1,234.00」は日本の帳票では見ない
pub(crate) fn currencies() -> Vec<(&'static str, &'static str, &'static str, usize)> {
    vec![
        (ui::item!("yen").0, ui::item!("yen").1, "¥", 0),
        (ui::item!("dollar").0, ui::item!("dollar").1, "$", 2),
        (ui::item!("euro").0, ui::item!("euro").1, "€", 2),
        (ui::item!("pound").0, ui::item!("pound").1, "£", 2),
        (ui::item!("won").0, ui::item!("won").1, "₩", 0),
        (ui::item!("yuan").0, ui::item!("yuan").1, "¥", 2),
        (ui::item!("no_symbol").0, ui::item!("no_symbol").1, "", 0),
    ]
}

/// 通貨の書式コードを組む。**記号は帳票のお金、並びは読む人の言語。**
///
/// `pattern` は `kumihan::datetime_names` の `currency_pattern`
/// (0=記号n / 1=n記号 / 2=記号␣n / 3=n␣記号)。独語は 3 なので
/// `#,##0.00 "€"`、日本語は 0 なので `"¥"#,##0` になる。
///
/// **記号は引用符で包む。** Excel がそう書く綴りで、包まないと
/// 記号によっては書式の記号と紛れる(`$` は Excel では特別な字)
pub(crate) fn currency_code(symbol: &str, decimals: usize, pattern: u8) -> String {
    let num = if decimals == 0 {
        "#,##0".to_string()
    } else {
        format!("#,##0.{}", "0".repeat(decimals))
    };
    if symbol.is_empty() {
        return num;
    }
    let sym = format!("\"{symbol}\"");
    match pattern {
        1 => format!("{num}{sym}"),
        2 => format!("{sym} {num}"),
        3 => format!("{num} {sym}"),
        _ => format!("{sym}{num}"),
    }
}

/// 日付の書式の候補。**(鍵, 見出し, 書式コード)**。
///
/// 見出しは**その書式で描いた見本そのもの**にする。「長い日付
/// (2026年8月6日)」のように例を焼き付けると、独語の人に日本語の日付を
/// 約束することになる(2026-08-10 に訳者4人が指摘した形)。
/// **描いた結果を見出しにすれば、見出しは嘘をつきようがない。**
///
/// 書式には `[$-407]` のように**地域を書き込む**。残さないと、開いた人の
/// 環境しだいで別の月名が出る — その帳票が何語で書かれたかを持たせる
/// (docs/sekkei/calc.ja.md「月名・曜日名は書式コードの地域から引く」)。
pub(crate) fn date_formats() -> Vec<(&'static str, String, String)> {
    let n = book::datetime_names::names(ui::language());
    let tag = format!("[$-{:x}]", n.lcid);
    // 見本は 2026-08-06(木)。通し番号 46240
    let show = |code: &str| {
        book::format_value(&book::Value::Number(46240.0), Some(code), false)
    };
    let rows: Vec<(&'static str, String)> = vec![
        (ui::item!("short_date").0, format!("{tag}{}", n.short_date)),
        (ui::item!("long_date").0, format!("{tag}{}", n.long_date)),
        (ui::item!("month_year").0, format!("{tag}mmmm yyyy")),
        (ui::item!("weekday_only").0, format!("{tag}dddd")),
        (ui::item!("time").0, "h:mm:ss".to_string()),
    ];
    rows.into_iter()
        .map(|(k, code)| {
            let sample = show(&code);
            (k, format!("{} — {}", ui::tr(k), sample), code)
        })
        .collect()
}

pub(crate) fn numfmts() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    vec![
        row(ui::item!("general"), None),
        row(ui::item!("number_1234_56"), Some("0.00")),
        row(ui::item!("thousands_separator_1_234"), Some("#,##0")),
        // **記号を見出しに書かない。** 「通貨 (¥1,234)」と出すと、
        // 独語の人に ¥ を約束することになる。押すと通貨を選ぶ一覧が開く
        row(ui::item!("currency_2"), None),
        row(ui::item!("percentage_12_34"), Some("0.00%")),
        row(ui::item!("scientific_1_23e_04"), Some("0.00E+00")),
        row(ui::item!("date_2"), None),
                row(ui::item!("text_2"), Some("@")),
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
        ui::item!("sentence_case"),
        ui::item!("lowercase"),
        ui::item!("uppercase"),
        ui::item!("capitalise_each_word"),
        ui::item!("toggle_case"),
    ]
}

/// 選んだ変え方で文字列を変換する(Unicode の upper/lower に従う)
pub(crate) fn change_case(t: &str, mode: &str) -> String {
    match mode {
        "lowercase" => t.to_lowercase(),
        "uppercase" => t.to_uppercase(),
        "sentence_case" => {
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
        "capitalise_each_word" => {
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
        "toggle_case" => t
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
pub(crate) fn border_styles() -> Vec<(&'static str, &'static str, book::BStyle)> {
    use book::BStyle;
    vec![
        row(ui::item!("thin_solid_default"), BStyle::Thin),
        row(ui::item!("hairline"), BStyle::Hair),
        row(ui::item!("dotted"), BStyle::Dotted),
        row(ui::item!("dashed"), BStyle::Dashed),
        row(ui::item!("dash_dot"), BStyle::DashDot),
        row(ui::item!("dash_dot_dot"), BStyle::DashDotDot),
        row(ui::item!("medium_solid"), BStyle::Medium),
        row(ui::item!("medium_dashed"), BStyle::MediumDashed),
        row(ui::item!("medium_dash_dot"), BStyle::MediumDashDot),
        row(ui::item!("medium_dash_dot_dot"), BStyle::MediumDashDotDot),
        row(ui::item!("thick_solid"), BStyle::Thick),
        row(ui::item!("double_line"), BStyle::Double),
    ]
}

/// 罫線を掛ける**場所**の9種(罫線パレットの見出し・ツールチップ)。
/// 太さ・線種・色はペンだけが決める — ここは場所しか言わない。
/// **鍵は日本語のまま** — [`Calc::apply_borders`] の照合はこの鍵で行う
/// (見出しだけが訳される)
pub(crate) fn border_kinds() -> Vec<(&'static str, &'static str)> {
    vec![
        ui::item!("bottom_border"),
        ui::item!("top_border"),
        ui::item!("left_border"),
        ui::item!("right_border"),
        ui::item!("outline"),
        ui::item!("all_borders_grid"),
        ui::item!("inside_vertical_border"),
        ui::item!("inside_horizontal_border"),
        ui::item!("no_border"),
    ]
}

/// 罫線の場所の鍵から、画面に出す見出しを引く。表に無ければ鍵をそのまま返す
pub(crate) fn border_kind_label(key: &str) -> String {
    border_kinds()
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, l)| l.to_string())
        .unwrap_or_else(|| key.to_string())
}

/// 保護中に許す操作の見出し。**中身は sheet 側**
/// ([`kumihan::book::ProtectAllow::items`])— sheet は zip と quick-xml しか
/// 要らない器なので、訳は**出す側のここ**で当てる。日本語の名前がそのまま鍵
/// (入切の照合は sheet に渡る)。並びが食い違わないことは tests.rs が見張る
pub(crate) fn protect_allows() -> Vec<(&'static str, &'static str)> {
    vec![
        ui::item!("moving_shapes_pictures"),
        ui::item!("select_locked_cells"),
        ui::item!("select_unlocked_cells"),
        ui::item!("format_cells"),
        ui::item!("format_columns"),
        ui::item!("format_rows"),
        ui::item!("insert_columns"),
        ui::item!("insert_rows"),
        ui::item!("insert_hyperlinks"),
        ui::item!("delete_columns"),
        ui::item!("delete_rows"),
        ui::item!("sort_2"),
        ui::item!("use_autofilter"),
        ui::item!("use_pivottable"),
    ]
}

/// 許す操作の鍵から見出しを引く。表に無ければ鍵をそのまま返す
pub(crate) fn protect_allow_label(key: &str) -> String {
    protect_allows()
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, l)| l.to_string())
        .unwrap_or_else(|| key.to_string())
}

/// 許す操作の要約 — 「14 のうち 3 つを許しています」。
/// ファイルのページの保護の面に出します。全部の名前を並べると1行に入りません
pub(crate) fn protect_allow_summary(a: &book::ProtectAllow) -> String {
    let items = a.items();
    let on = items.iter().filter(|(_, v)| *v).count();
    if on == 0 {
        ui::t!("nothing_allowed").to_string()
    } else {
        ui::tf!("allowed_2", items.len(), on).to_string()
    }
}

/// 配色(テーマ色の組)の見出し。**中身は sheet 側**([`kumihan::book::theme::SCHEMES`])。
/// 鍵はそちらの名前そのもの。並びが食い違わないことは tests.rs が見張る
pub(crate) fn color_schemes() -> Vec<(&'static str, &'static str)> {
    vec![
        // 「Office」は色の組の固有名 — 訳す言葉ではない
        ("Office", "Office"),
        ui::item!("warm"),
        ui::item!("cool"),
        ui::item!("ink"),
    ]
}

/// 配色の鍵から見出しを引く。表に無ければ鍵をそのまま返す
pub(crate) fn color_scheme_label(key: &str) -> String {
    color_schemes()
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, l)| l.to_string())
        .unwrap_or_else(|| key.to_string())
}

pub(crate) fn font_colors() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    vec![
        row(ui::item!("automatic"), None),
        row(ui::item!("black"), Some("1B1B1B")),
        row(ui::item!("red"), Some("C00000")),
        row(ui::item!("orange"), Some("ED7D31")),
        row(ui::item!("yellow"), Some("FFC000")),
        row(ui::item!("green"), Some("70AD47")),
        row(ui::item!("blue"), Some("4472C4")),
        row(ui::item!("navy"), Some("1F4E79")),
        row(ui::item!("purple"), Some("7030A0")),
        row(ui::item!("grey"), Some("7F7F7F")),
        row(ui::item!("white"), Some("FFFFFF")),
    ]
}

/// 塗りつぶしのパレット(帳票で使う薄い色を先に)
pub(crate) fn fill_colors() -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    vec![
        row(ui::item!("no_colour"), None),
        row(ui::item!("light_yellow"), Some("FFF2CC")),
        row(ui::item!("light_blue"), Some("DEEAF6")),
        row(ui::item!("light_green"), Some("E2EFDA")),
        row(ui::item!("light_orange"), Some("FCE4D6")),
        row(ui::item!("light_grey"), Some("D9D9D9")),
        row(ui::item!("yellow"), Some("FFC000")),
        row(ui::item!("orange"), Some("ED7D31")),
        row(ui::item!("green"), Some("70AD47")),
        row(ui::item!("blue"), Some("4472C4")),
        row(ui::item!("grey"), Some("7F7F7F")),
    ]
}

/// 表のスタイル(見出し行の色と、縞の色)。**表を作るときに選ぶ。**
///
/// 前は色が1組に決め打ちで、緑の帳票しか作れなかった(2026-08-12 まで)。
/// 見た目は**書式として掛かる** — 表を外しても残るし、後から掛け直せる
/// (SEKKEI「表そのもの」の節)。だから色の組だけを持てばよい。
///
/// 色名は既に訳のある語をそのまま鍵にしている(新しい文言を増やさない)。
pub(crate) fn table_styles() -> Vec<(&'static str, &'static str, TableStyle)> {
    vec![
        row(ui::item!("green"), TableStyle::new("D5E8DC", "F1F6F3")),
        row(ui::item!("blue"), TableStyle::new("D6E4F0", "EEF4FA")),
        row(ui::item!("orange"), TableStyle::new("FCE4D6", "FDF2EC")),
        row(ui::item!("red"), TableStyle::new("F8D7DA", "FCEEEF")),
        row(ui::item!("purple"), TableStyle::new("E4DCEF", "F3F0F8")),
        row(ui::item!("grey"), TableStyle::new("E7E9EB", "F4F5F6")),
        // 色を敷かない。**罫線と太字だけ**で組む帳票のため
        row(ui::item!("borders_only"), TableStyle { header: None, band: None }),
    ]
}

/// 表のスタイル1つ。`None` は「色を敷かない」
#[derive(Clone, Copy)]
pub(crate) struct TableStyle {
    pub(crate) header: Option<&'static str>,
    pub(crate) band: Option<&'static str>,
}

impl TableStyle {
    const fn new(header: &'static str, band: &'static str) -> Self {
        Self { header: Some(header), band: Some(band) }
    }
}

pub(crate) fn cell_styles() -> Vec<CellStyle> {
    let f: Vec<CellStyle> = vec![
    row(ui::item!("normal"), |f| *f = CellFormat::default()),
    // **見出しは4段**(2026-08-20 発注者「Excel が 見出し1〜4 を持つので
    // あれば、そうしていいのでは」)。前は1段だけで、章と節を書き分け
    // られませんでした。
    //
    // *色は Excel の写しではありません。* いまの見出しが使っていた緑
    // (画面の帯の色)のまま、段が下がるほど字を小さく、線を細くします。
    // 「MS がそうだから」は理由にしない決めです
    row(ui::item!("heading_1"), |f| {
        f.bold = true;
        f.size_c = Some(1400);
        f.color = Some("1B6E3C".into());
        f.fill = Some("D5E8DC".into());
        f.borders.bottom = book::Edge::line(book::BStyle::Medium, None);
    }),
    row(ui::item!("heading_2"), |f| {
        f.bold = true;
        f.size_c = Some(1200);
        f.color = Some("1B6E3C".into());
        f.fill = Some("D5E8DC".into());
        f.borders.bottom = book::Edge::THIN;
    }),
    row(ui::item!("heading_3"), |f| {
        f.bold = true;
        f.color = Some("1B6E3C".into());
        f.borders.bottom = book::Edge::THIN;
    }),
    row(ui::item!("heading_4"), |f| {
        f.bold = true;
        f.color = Some("1B6E3C".into());
    }),
    row(ui::item!("title_style"), |f| {
        f.bold = true;
        f.size_c = Some(1600);
        f.color = Some("1B6E3C".into());
    }),
    row(ui::item!("good"), |f| {
        f.fill = Some("C6EFCE".into());
        f.color = Some("006100".into());
    }),
    row(ui::item!("bad"), |f| {
        f.fill = Some("FFC7CE".into());
        f.color = Some("9C0006".into());
    }),
    row(ui::item!("neutral"), |f| {
        f.fill = Some("FFEB9C".into());
        f.color = Some("9C6500".into());
    }),
    row(ui::item!("note"), |f| {
        f.fill = Some("FFFFCC".into());
        f.borders = Borders::ALL;
    }),
    row(ui::item!("calculation"), |f| {
        f.italic = true;
        f.fill = Some("F2F2F2".into());
        f.color = Some("7F7F7F".into());
    }),
    row(ui::item!("currency"), |f| f.number_format = Some("¥#,##0".into())),
    row(ui::item!("percent"), |f| f.number_format = Some("0.0%".into())),
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
pub(crate) fn cond_kind_name(k: &book::CondKind) -> String {
    use book::CondKind;
    match k {
        CondKind::Cmp(op, v) => format!("{} {}", ui::t!("compare_value"), format_args!("{op:?} {v}")),
        CondKind::Between(lo, hi, false) => ui::tf!("between", lo, hi).to_string(),
        CondKind::Between(lo, hi, true) => ui::tf!("outside", lo, hi).to_string(),
        CondKind::Text(t) => ui::tf!("contains", t).to_string(),
        CondKind::Dup(false) => ui::t!("duplicate_values").to_string(),
        CondKind::Dup(true) => ui::t!("unique_values").to_string(),
        CondKind::Top(n, false) => ui::tf!("top_2", n).to_string(),
        CondKind::Top(n, true) => ui::tf!("bottom_2", n).to_string(),
        CondKind::Avg(false) => ui::t!("above_average").to_string(),
        CondKind::Avg(true) => ui::t!("below_average").to_string(),
        CondKind::Bar(_) => ui::t!("data_bar").to_string(),
        CondKind::Scale(..) => ui::t!("colour_scale").to_string(),
        CondKind::Icons(_) => ui::t!("icon_set").to_string(),
        // 式は左上を錨にした原文。一覧では `=` を付けて見せる(編集欄と同じ形)
        CondKind::Formula(f) => ui::tf!("formula", f).to_string(),
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

/// Alt のキーヒントの札の行き先(2026-08-13、台帳「Alt キーヒント」)
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum HintTo {
    /// リボンの段(番号)
    Tab(usize),
    /// 段の中のボタン(命令の id)
    Cmd(&'static str),
}

/// コメントの一覧の並べ方(台帳 第2便の [中]、2026-08-13)。
///
/// 本家は「日付/著者/グループ/ステータス」の4択。**グループは置かない** —
/// あれは文書サーバーに居る利用者の組の話で、手元のファイルには無い概念
/// (「所有者・アップロード日」を作らなかったのと同じ理由)。
/// 代わりに**場所**を入れた。帳票を上から追うときはこれがいちばん要る
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CommentSort {
    /// シート順 → セル順(既定)
    Place,
    /// 筋の頭を書いた日
    When,
    /// 筋の頭を書いた人
    Who,
    /// 未解決 → 解決済み
    Done,
}

impl CommentSort {
    /// 板の頭に出す札。**(鍵, 見出し)の組** — 鍵=日本語で引き当て、
    /// 見出しだけが画面の言語になる(`ui::item!` の作法)
    pub(crate) fn label(self) -> (&'static str, &'static str) {
        match self {
            CommentSort::Place => ui::item!("place"),
            CommentSort::When => ui::item!("date"),
            CommentSort::Who => ui::item!("author"),
            CommentSort::Done => ui::item!("status"),
        }
    }
}

/// コメントの一覧の板(開いていれば並べ方を持つ)。
pub(crate) struct CommentList {
    pub(crate) sort: CommentSort,
    /// 逆順に並べるか
    pub(crate) desc: bool,
}

impl Default for CommentList {
    fn default() -> Self {
        Self { sort: CommentSort::Place, desc: false }
    }
}

/// 一覧の1行。**画面にも試験にも同じ物を使う**ので、並べ替えは
/// 描くところではなく [`sort_comments`] にある
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct CommentRow {
    /// どのシートか。**一覧はブック全体**(削除の「すべて」と範囲を揃える)
    pub(crate) sheet: usize,
    pub(crate) at: Pos,
    /// 筋の頭を書いた人(空 = 名乗りが無い)
    pub(crate) who: String,
    /// 筋の頭の日(ISO8601 の綴りのまま。空 = 打ったばかり)
    pub(crate) when: String,
    pub(crate) done: bool,
    /// 筋の頭の文
    pub(crate) text: String,
    /// 返信の数(頭は数えない)
    pub(crate) replies: usize,
}

/// 一覧を並べ替える。
///
/// **空の欄はいつも最後**(降順でも先頭に来ない)— スライサーの「(空白)」と
/// 同じ決め。名乗りや日付の無い筋が先頭に並ぶと、一覧が読めなくなる。
///
/// 同じ値のときは必ず**場所**で決める。並びが揺れると、消す相手を
/// 押し間違える
pub(crate) fn sort_comments(rows: &mut [CommentRow], sort: CommentSort, desc: bool) {
    rows.sort_by(|a, b| {
        let place = (a.sheet, a.at).cmp(&(b.sheet, b.at));
        // 空を後ろへ回す比べ方(空同士は引き分け)
        let blank_last = |x: &str, y: &str| match (x.is_empty(), y.is_empty()) {
            (true, true) => std::cmp::Ordering::Equal,
            // 空は「いつも最後」— desc で引っくり返さないよう、ここでは
            // 印だけ返して下で扱う
            _ => x.is_empty().cmp(&y.is_empty()),
        };
        let (blank, main) = match sort {
            CommentSort::Place => (std::cmp::Ordering::Equal, place),
            CommentSort::When => (blank_last(&a.when, &b.when), a.when.cmp(&b.when)),
            CommentSort::Who => (blank_last(&a.who, &b.who), a.who.cmp(&b.who)),
            CommentSort::Done => (std::cmp::Ordering::Equal, a.done.cmp(&b.done)),
        };
        if blank != std::cmp::Ordering::Equal {
            return blank; // 空はここで決まる(逆順にしない)
        }
        let main = if desc { main.reverse() } else { main };
        main.then(place)
    });
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
    // ---- 見た目(2026-08-13、台帳「スライサー設定タブ」)----
    /// 板の幅(px)。既定は 190
    pub(crate) w: f32,
    /// 板の高さ(px)。値が入りきらなければ中で送る
    pub(crate) h: f32,
    /// **幅と高さの比を保つか。** 片方を変えるともう片方も同じ率で動く
    pub(crate) ratio: bool,
    /// ボタンを何列に並べるか(1〜4)。値が多い列を短く収めるため
    pub(crate) cols: u32,
    /// スタイル([`slicer_styles`] の番号)
    pub(crate) style: usize,
    /// 置き場所(格子の面の px の左上)。`None` = 右から順に自動で並べる
    pub(crate) at: Option<(f32, f32)>,
    /// **日付の粒**(タイムライン。2026-08-22 の D群)。空 = 値そのもの。
    /// 「月」「四半期」「年」を入れると、日付の列を束にまとめて並べます
    pub(crate) grain: String,
    /// **つないだピボットの名前**(レポートの接続。2026-08-21 の D群)。
    /// 空 = どのピボットにも繋がっていません。繋いだピボットは、この
    /// スライサーを押すたびに同じ絞りで作り直します
    pub(crate) pivots: Vec<String>,
}

/// 板の既定の大きさ。**幅は前からの 190px**(変えると、いま開いている
/// 帳票の見え方が黙って変わる)
pub(crate) const SLICER_W: f32 = 190.0;
pub(crate) const SLICER_H: f32 = 220.0;

impl Default for Slicer {
    fn default() -> Self {
        Self {
            col: 0,
            sel: Default::default(),
            multi: false,
            desc: false,
            hide_empty: false,
            w: SLICER_W,
            h: SLICER_H,
            ratio: false,
            grain: String::new(),
            pivots: Vec::new(),
            cols: 1,
            style: 0,
            at: None,
        }
    }
}

/// スライサーのスタイル(見出しの地・選んだ値の地・縁)。
///
/// **画面で描き分けられる物だけ**を並べる(柄を18種から6種に絞ったのと
/// 同じ線)。本家の 14 種は縞や角丸の違いを含むが、こちらは色の組で足りる。
/// 組は (鍵, 見出し, 見出しの地, 選んだ値の地, 縁)
pub(crate) fn slicer_styles() -> Vec<(&'static str, &'static str, u32, u32, u32)> {
    let row = |(k, l): (&'static str, &'static str), a, b, c| (k, l, a, b, c);
    vec![
        row(ui::item!("green"), 0xFFFFFF, 0xBBD9EA, 0x1B6E3C),
        row(ui::item!("blue"), 0xEEF4FA, 0xBBD9EA, 0x2E6DA4),
        row(ui::item!("orange"), 0xFDF2EC, 0xF6C99B, 0xB86A22),
        row(ui::item!("purple"), 0xF3F0F8, 0xD3C6EA, 0x6E4FA3),
        row(ui::item!("grey"), 0xF4F5F6, 0xD5DADE, 0x6B7680),
    ]
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
        out.push(ui::t!("blank").to_string());
    }
    (out, cut)
}

/// 図形ギャラリー(台帳 第2便の [中]、2026-08-13)。
///
/// **分類は本家の並び。** 載せるのは `kumihan::book::can_draw` が
/// 「その形として描ける」と答えるものだけ — できないものを、
/// できるように見せない(`cube` や `heart` を並べれば、四角が置かれる)。
/// 組は (鍵=日本語, 見出し)。**引き当ては鍵**で、見出しだけが画面の言語になる。
pub(crate) fn shape_gallery(cat: &str) -> Vec<(&'static str, &'static str)> {
    match cat {
        "basic_shapes" => vec![
            ui::item!("rectangle"),
            ui::item!("rounded_rectangle"),
            ui::item!("ellipse"),
            ui::item!("triangle"),
            ui::item!("right_triangle"),
            ui::item!("parallelogram"),
            ui::item!("trapezium"),
            ui::item!("diamond"),
            ui::item!("pentagon"),
            ui::item!("hexagon"),
            ui::item!("octagon"),
            ui::item!("cross"),
        ],
        "block_arrows" => vec![
            ui::item!("right_arrow"),
            ui::item!("left_arrow"),
            ui::item!("up_arrow"),
            ui::item!("down_arrow"),
            ui::item!("left_right_arrow"),
            ui::item!("up_down_arrow"),
        ],
        "equation_shapes" => vec![
            ui::item!("plus_sign"),
            ui::item!("minus_sign"),
            ui::item!("multiply_sign"),
            ui::item!("equals_sign"),
            ui::item!("not_equal_sign"),
        ],
        "flowchart" => vec![
            ui::item!("process_step"),
            ui::item!("decision"),
            ui::item!("data"),
            ui::item!("terminator"),
            ui::item!("document_shape"),
            ui::item!("connector"),
        ],
        "stars_ribbons" => vec![
            ui::item!("4_point_star"),
            ui::item!("5_point_star"),
            ui::item!("6_point_star"),
            ui::item!("8_point_star"),
        ],
        "callouts" => vec![
            ui::item!("rectangular_callout"),
            ui::item!("oval_callout"),
        ],
        "line_border" => vec![ui::item!("straight_line"), ui::item!("free_shape_made_points")],
        _ => Vec::new(),
    }
}

/// 分類の鍵 → 画面の見出し(2段目の題に出す)
pub(crate) fn shape_cat_label(cat: &str) -> &'static str {
    match cat {
        "block_arrows" => ui::t!("block_arrows"),
        "equation_shapes" => ui::t!("equation_shapes"),
        "flowchart" => ui::t!("flowchart"),
        "stars_ribbons" => ui::t!("stars_ribbons"),
        "callouts" => ui::t!("callouts"),
        "line_border" => ui::t!("line_border"),
        _ => ui::t!("basic_shapes"),
    }
}

/// 図形の鍵 → (prstGeom の名前, 画面の見出し)。
/// **知らない鍵は四角**(一覧から来る限り起こらないが、黙って落とさない)
pub(crate) fn shape_kind(v: &str) -> (&'static str, &'static str) {
    match v {
        "rounded_rectangle" => ("roundRect", ui::t!("rounded_rectangle")),
        "ellipse" => ("ellipse", ui::t!("ellipse")),
        "triangle" => ("triangle", ui::t!("triangle")),
        "right_triangle" => ("rtTriangle", ui::t!("right_triangle")),
        "parallelogram" => ("parallelogram", ui::t!("parallelogram")),
        "trapezium" => ("trapezoid", ui::t!("trapezium")),
        "diamond" => ("diamond", ui::t!("diamond")),
        "pentagon" => ("pentagon", ui::t!("pentagon")),
        "hexagon" => ("hexagon", ui::t!("hexagon")),
        "octagon" => ("octagon", ui::t!("octagon")),
        "cross" => ("plus", ui::t!("cross")),
        "right_arrow" => ("rightArrow", ui::t!("right_arrow")),
        "left_arrow" => ("leftArrow", ui::t!("left_arrow")),
        "up_arrow" => ("upArrow", ui::t!("up_arrow")),
        "down_arrow" => ("downArrow", ui::t!("down_arrow")),
        "left_right_arrow" => ("leftRightArrow", ui::t!("left_right_arrow")),
        "up_down_arrow" => ("upDownArrow", ui::t!("up_down_arrow")),
        "plus_sign" => ("mathPlus", ui::t!("plus_sign")),
        "minus_sign" => ("mathMinus", ui::t!("minus_sign")),
        "multiply_sign" => ("mathMultiply", ui::t!("multiply_sign")),
        "equals_sign" => ("mathEqual", ui::t!("equals_sign")),
        "not_equal_sign" => ("mathNotEqual", ui::t!("not_equal_sign")),
        "process_step" => ("flowChartProcess", ui::t!("process_step")),
        "decision" => ("flowChartDecision", ui::t!("decision")),
        "data" => ("flowChartInputOutput", ui::t!("data")),
        "terminator" => ("flowChartTerminator", ui::t!("terminator")),
        "document_shape" => ("flowChartDocument", ui::t!("document_shape")),
        "connector" => ("flowChartConnector", ui::t!("connector")),
        "4_point_star" => ("star4", ui::t!("4_point_star")),
        "5_point_star" => ("star5", ui::t!("5_point_star")),
        "6_point_star" => ("star6", ui::t!("6_point_star")),
        "8_point_star" => ("star8", ui::t!("8_point_star")),
        "rectangular_callout" => ("wedgeRectCallout", ui::t!("rectangular_callout")),
        "oval_callout" => ("wedgeEllipseCallout", ui::t!("oval_callout")),
        "straight_line" => ("line", ui::t!("straight_line")),
        "free_shape_made_points" => ("path", ui::t!("free_shape_made_points")),
        _ => ("rect", ui::t!("rectangle")),
    }
}

/// 塗りの柄(xlsx の patternType)。
///
/// **画面で描き分けられる柄だけを並べる。** GPUI が持つ背景は
/// 単色・線形グラデーション・斜線・市松の4つで、横線と縦線と斜め格子は
/// 描き分けられない — 並べれば「選べるのに同じに見える」ことになる。
/// 網の %% は**濃さの混色**で見せる: セルの大きさでは細かい点は目に
/// 解けず、Excel も同じ見え方になる(方向を持つ柄だけが別物)。
///
/// 読むほうは xlsx のどの柄も受ける(往復は保つ) — 出せるものを絞るだけ。
/// 組は (鍵=日本語, 見出し)。引き当ては `pattern_kind` が持つ
pub(crate) fn fill_patterns() -> Vec<(&'static str, &'static str)> {
    vec![
        ui::item!("6_25_shading"),
        ui::item!("12_5_shading"),
        ui::item!("25_shading"),
        ui::item!("50_shading"),
        ui::item!("diagonal_stripes"),
        ui::item!("checkerboard"),
    ]
}

/// 柄の鍵 → xlsx の patternType
pub(crate) fn pattern_kind(v: &str) -> Option<&'static str> {
    Some(match v {
        "6_25_shading" => "gray0625",
        "12_5_shading" => "gray125",
        "25_shading" => "lightGray",
        "50_shading" => "mediumGray",
        "diagonal_stripes" => "darkUp",
        "checkerboard" => "darkGrid",
        _ => return None,
    })
}

/// グラデーションの向き。**角度で持つ**(xlsx の degree)
pub(crate) fn grad_dirs() -> Vec<(&'static str, &'static str)> {
    vec![
        ui::item!("horizontal_left_right"),
        ui::item!("vertical_top_bottom"),
        ui::item!("diagonal_top_left_bottom"),
        ui::item!("diagonal_bottom_left_top"),
    ]
}

/// 向きの鍵 → (角度×100, 放射か)。
/// **放射は並べない** — GPUI に放射の背景が無く、線形で代用すると
/// 選んだ物と見える物が食い違う。読みは受ける(往復は保つ)
pub(crate) fn grad_dir_of(v: &str) -> Option<(i32, bool)> {
    Some(match v {
        "horizontal_left_right" => (0, false),
        "vertical_top_bottom" => (9000, false),
        "diagonal_top_left_bottom" => (4500, false),
        "diagonal_bottom_left_top" => (31500, false),
        _ => return None,
    })
}

/// セルの塗りを GPUI の背景に直す。**返りは(下地, 柄の重ね)**。
///
/// 柄は前景色で下地の上に敷く(データバーと同じ重ね方)。
/// GPUI が持つ背景は単色・線形グラデーション・斜線・市松の4つなので、
/// **描き分けられない柄は濃さの混色に落とす** — 方向を持たない網は
/// セルの大きさでは点が目に解けず、混色のほうが実物に近い。
/// 横線・縦線のような方向のある柄は一覧に並べていないが、Excel の帳票には
/// 入ってくるので、ここへ来たら濃さで見せる(色は正しく、粗さだけが違う)。
pub(crate) fn cell_background(
    f: &book::CellFormat,
    base: gpui::Rgba,
) -> (gpui::Background, Option<gpui::Background>) {
    use gpui::{checkerboard, linear_gradient, pattern_slash};
    // グラデーションが先(柄とは排他)
    if let Some(g) = &f.fill_grad {
        let col = |i: usize, dflt: gpui::Rgba| {
            g.stops.get(i).map(|(_, c)| hex(c)).unwrap_or(dflt)
        };
        let from = col(0, base);
        let to = col(g.stops.len().saturating_sub(1), gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
        // xlsx の 0 度は「左から右」、GPUI の 0 度は「上へ」で時計回り
        let angle = (g.degree_c as f32 / 100.0 + 90.0).rem_euclid(360.0);
        return (
            linear_gradient(
                angle,
                gpui::linear_color_stop(from, 0.0),
                gpui::linear_color_stop(to, 1.0),
            ),
            None,
        );
    }
    let Some(p) = &f.fill_pattern else { return (base.into(), None) };
    // 柄の下地は bgColor(無ければ白)、柄そのものは前景色(= fill)
    let bg = f.fill_bg.as_deref().map(hex).unwrap_or(gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
    let fg = f.fill.as_deref().map(hex).unwrap_or(gpui::Rgba { r: 0.5, g: 0.5, b: 0.5, a: 1.0 });
    match p.as_str() {
        // 斜めの縞。GPUI の斜線は1方向だけなので、上がりも下がりもこれで見せる
        "darkUp" | "darkDown" | "lightUp" | "lightDown" => {
            (bg.into(), Some(pattern_slash(fg, 1.0, 3.0)))
        }
        "darkGrid" | "darkTrellis" | "lightGrid" | "lightTrellis" => {
            (bg.into(), Some(checkerboard(fg, 4.0)))
        }
        // 濃さで見せる柄(方向を持たないもの、と描き分けられないもの)
        other => {
            let t = match other {
                "gray0625" => 0.0625,
                "gray125" => 0.125,
                "lightGray" => 0.25,
                "mediumGray" => 0.5,
                "darkGray" => 0.75,
                // 方向のある柄(横線・縦線)は描き分けられない。濃さで見せる
                "lightHorizontal" | "lightVertical" => 0.25,
                "darkHorizontal" | "darkVertical" => 0.5,
                _ => 0.5,
            };
            let mix = |a: f32, b: f32| a * (1.0 - t) + b * t;
            (
                gpui::Rgba {
                    r: mix(bg.r, fg.r),
                    g: mix(bg.g, fg.g),
                    b: mix(bg.b, fg.b),
                    a: 1.0,
                }
                .into(),
                None,
            )
        }
    }
}
