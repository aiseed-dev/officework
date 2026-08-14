//! main.rs からの純移動(2026-08-06 の分割)。挙動は変えない。

use crate::*;

pub(crate) use ops::{font_data, image_px};

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
/// 字の大きさ(pt)を画面の px にする。**96dpi の標準どおり 96/72**。
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

// 改名の参照書き換え(rename_refs_in / rename_sheet_refs)は 2026-08-12 に
// sheet::model::refs へ**純移動**した — Python(pysheet)の改名でも式が
// 追随するように。呼び側(picks.rs・tests.rs)の名前が変わらないよう再輸出する
#[allow(unused_imports)] // rename_refs_in は tests.rs だけが使う
pub(crate) use sheet::model::{rename_refs_in, rename_sheet_refs};

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

/// 分類の耳。「すべて」+ funcs.rs の分類。**日本語のまま持つ** —
/// これは絞り込みの照合に使う鍵で、画面に出すときだけ訳す
pub(crate) const FN_GROUPS: &[&str] = &["すべて", "数学/三角", "統計", "文字列操作", "論理", "日付/時刻", "検索/行列", "財務", "情報"];

/// 分類の耳に出す語。**変数を `ui::tr` に渡さず、1つずつ literal で書く。**
///
/// 前は `ui::tr(g)` と鍵を変数で渡していた。動きはするが、**文言の門番は
/// 印の付いた literal しか見られない**ので、訳が無くても誰も気づかない。
/// 実際、9 個のうち5個(すべて・数学・文字列・論理・財務)が未訳のまま
/// 画面に出ていた(2026-08-11、関数の説明を各言語に繋いだときに見つけた)。
pub(crate) fn fn_group_label(key: &'static str) -> &'static str {
    match key {
        "すべて" => ui::t!("すべて"),
        // **名前はリボンの族のボタンに揃える。** 同じ関数の集まりなのに
        // 耳が「数学」でボタンが「数学/三角」では、同じ製品の中で
        // 2つの名前を持つことになる。はじめ短い名前を別に作ってしまい、
        // 訳も別に頼んでいた(2026-08-11、下請けの指摘で気づいた)。
        // 一語の鍵は他の意味と衝突もする — 「日付」は数値の書式の
        // 見出し、「検索」は検索コマンド(英語は "Find")
        "数学/三角" => ui::t!("数学/三角"),
        "統計" => ui::t!("統計"),
        "文字列操作" => ui::t!("文字列操作"),
        "論理" => ui::t!("論理"),
        "日付/時刻" => ui::t!("日付/時刻"),
        "検索/行列" => ui::t!("検索/行列"),
        "財務" => ui::t!("財務"),
        "情報" => ui::t!("情報"),
        other => other,
    }
}

/// 分類の耳から「その族の一覧」を開くときのコマンド id。
///
/// **綴りは `FN_GROUPS` が正。** 前はこの照合が picks.rs に直に書いてあり、
/// 綴りがずれると黙って既定の `fn-lookup` に落ちて**別の一覧が出る**
/// (2026-08-11、「日付」を「日付・時刻」に広げたときに踏みかけた)。
/// 1箇所に集めて、試験が `FN_GROUPS` と突き合わせられるようにした。
pub(crate) fn fn_group_cmd(group: &str) -> &'static str {
    match group {
        "統計" | "数学/三角" => "fn-math",
        "財務" => "fn-financial",
        "日付/時刻" => "fn-datetime",
        "文字列操作" => "fn-text",
        "論理" => "fn-logical",
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
        "{{\"headers\":[{}],\"rows\":[{}],\"index\":[{}],\"columns\":[{}],\"value\":\"{}\",\"agg\":\"{}\",\"totals\":{},\"subtotals\":{},\"blank_rows\":{},\"compact\":{},\"hide\":[{hides}],\"vfilter\":{vf},\"group\":[{groups}],\"show_as\":\"{sa}\",\"sort\":\"{so}\"}}",
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
        so = esc(&d.sort),
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

/// **すべて**(中身と書式)を貼る — 普通の Ctrl+V。このアプリでコピーした
/// 範囲だけが通る道で、控えたセルをそのまま置く。式の相対参照は `shift`。
///
/// 本家の普通の貼り付けは書式も運ぶ(発注者 2026-08-14)。値だけ・書式だけ・
/// 式だけは「形式を選択して貼り付け」の側にある — 分かれ道はそちら
pub(crate) fn paste_all_cells(
    s: &mut sheet::Sheet,
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
                        cell.formula = Some(sheet::model::offset_refs(f, r, c));
                    }
                    s.set(p, cell);
                }
                // 空のセルを貼るのも「貼る」— 元が空なら先も空にする
                // (書式は元のまま。中身だけ消す — 帳票の枠を壊さない)
                None => {
                    let fmt = s.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                    s.set(p, Cell { formula: None, value: sheet::Value::Empty, fmt });
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
        (ui::item!("円 (¥)").0, ui::item!("円 (¥)").1, "¥", 0),
        (ui::item!("ドル ($)").0, ui::item!("ドル ($)").1, "$", 2),
        (ui::item!("ユーロ (€)").0, ui::item!("ユーロ (€)").1, "€", 2),
        (ui::item!("ポンド (£)").0, ui::item!("ポンド (£)").1, "£", 2),
        (ui::item!("ウォン (₩)").0, ui::item!("ウォン (₩)").1, "₩", 0),
        (ui::item!("元 (¥)").0, ui::item!("元 (¥)").1, "¥", 2),
        (ui::item!("記号なし").0, ui::item!("記号なし").1, "", 0),
    ]
}

/// 通貨の書式コードを組む。**記号は帳票のお金、並びは読む人の言語。**
///
/// `pattern` は `sheet::datetime_names` の `currency_pattern`
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
    let n = sheet::datetime_names::names(ui::language());
    let tag = format!("[$-{:x}]", n.lcid);
    // 見本は 2026-08-06(木)。通し番号 46240
    let show = |code: &str| {
        sheet::model::format_value(&sheet::Value::Number(46240.0), Some(code), false)
    };
    let rows: Vec<(&'static str, String)> = vec![
        (ui::item!("短い日付").0, format!("{tag}{}", n.short_date)),
        (ui::item!("長い日付").0, format!("{tag}{}", n.long_date)),
        (ui::item!("年と月").0, format!("{tag}mmmm yyyy")),
        (ui::item!("曜日だけ").0, format!("{tag}dddd")),
        (ui::item!("時刻").0, "h:mm:ss".to_string()),
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
        row(ui::item!("一般"), None),
        row(ui::item!("数値 (1234.56)"), Some("0.00")),
        row(ui::item!("桁区切り (1,234)"), Some("#,##0")),
        // **記号を見出しに書かない。** 「通貨 (¥1,234)」と出すと、
        // 独語の人に ¥ を約束することになる。押すと通貨を選ぶ一覧が開く
        row(ui::item!("通貨…"), None),
        row(ui::item!("パーセント (12.34%)"), Some("0.00%")),
        row(ui::item!("指数 (1.23E+04)"), Some("0.00E+00")),
        row(ui::item!("日付…"), None),
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

/// 罫線を掛ける**場所**の9種(罫線パレットの見出し・ツールチップ)。
/// 太さ・線種・色はペンだけが決める — ここは場所しか言わない。
/// **鍵は日本語のまま** — [`Calc::apply_borders`] の照合はこの鍵で行う
/// (見出しだけが訳される)
pub(crate) fn border_kinds() -> Vec<(&'static str, &'static str)> {
    vec![
        ui::item!("下罫線"),
        ui::item!("上罫線"),
        ui::item!("左罫線"),
        ui::item!("右罫線"),
        ui::item!("外枠"),
        ui::item!("すべての罫線(格子)"),
        ui::item!("内側の縦線"),
        ui::item!("内側の横線"),
        ui::item!("罫線を消す"),
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
/// ([`sheet::model::ProtectAllow::items`])— sheet は zip と quick-xml しか
/// 要らない器なので、訳は**出す側のここ**で当てる。日本語の名前がそのまま鍵
/// (入切の照合は sheet に渡る)。並びが食い違わないことは tests.rs が見張る
pub(crate) fn protect_allows() -> Vec<(&'static str, &'static str)> {
    vec![
        ui::item!("図形・画像の操作"),
        ui::item!("ロックされたセルの選択"),
        ui::item!("ロックされていないセルの選択"),
        ui::item!("セルの書式設定"),
        ui::item!("列の書式設定"),
        ui::item!("行の書式設定"),
        ui::item!("列の挿入"),
        ui::item!("行の挿入"),
        ui::item!("ハイパーリンクの挿入"),
        ui::item!("列の削除"),
        ui::item!("行の削除"),
        ui::item!("並べ替え"),
        ui::item!("オートフィルターの使用"),
        ui::item!("ピボットテーブルの使用"),
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

/// 配色(テーマ色の組)の見出し。**中身は sheet 側**([`sheet::theme::SCHEMES`])。
/// 鍵はそちらの名前そのもの。並びが食い違わないことは tests.rs が見張る
pub(crate) fn color_schemes() -> Vec<(&'static str, &'static str)> {
    vec![
        // 「Office」は色の組の固有名 — 訳す言葉ではない
        ("Office", "Office"),
        ui::item!("暖色"),
        ui::item!("寒色"),
        ui::item!("墨"),
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

/// 表のスタイル(見出しの帯の色と、縞の色)。**表を作るときに選ぶ。**
///
/// 前は色が1組に決め打ちで、緑の帳票しか作れなかった(2026-08-12 まで)。
/// 見た目は**書式として掛かる** — 表を外しても残るし、後から掛け直せる
/// (SEKKEI「表そのもの」の節)。だから色の組だけを持てばよい。
///
/// 色名は既に訳のある語をそのまま鍵にしている(新しい文言を増やさない)。
pub(crate) fn table_styles() -> Vec<(&'static str, &'static str, TableStyle)> {
    vec![
        row(ui::item!("緑"), TableStyle::new("D5E8DC", "F1F6F3")),
        row(ui::item!("青"), TableStyle::new("D6E4F0", "EEF4FA")),
        row(ui::item!("橙"), TableStyle::new("FCE4D6", "FDF2EC")),
        row(ui::item!("赤"), TableStyle::new("F8D7DA", "FCEEEF")),
        row(ui::item!("紫"), TableStyle::new("E4DCEF", "F3F0F8")),
        row(ui::item!("灰"), TableStyle::new("E7E9EB", "F4F5F6")),
        // 色を敷かない。**罫線と太字だけ**で組む帳票のため
        row(ui::item!("枠線だけ"), TableStyle { header: None, band: None }),
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
            CommentSort::Place => ui::item!("場所"),
            CommentSort::When => ui::item!("日付"),
            CommentSort::Who => ui::item!("著者"),
            CommentSort::Done => ui::item!("状態"),
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
        row(ui::item!("緑"), 0xFFFFFF, 0xBBD9EA, 0x1B6E3C),
        row(ui::item!("青"), 0xEEF4FA, 0xBBD9EA, 0x2E6DA4),
        row(ui::item!("橙"), 0xFDF2EC, 0xF6C99B, 0xB86A22),
        row(ui::item!("紫"), 0xF3F0F8, 0xD3C6EA, 0x6E4FA3),
        row(ui::item!("灰"), 0xF4F5F6, 0xD5DADE, 0x6B7680),
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
        out.push(ui::t!("(空白)").to_string());
    }
    (out, cut)
}

/// 図形ギャラリー(台帳 第2便の [中]、2026-08-13)。
///
/// **分類は本家の並び。** 載せるのは `sheet::model::can_draw` が
/// 「その形として描ける」と答えるものだけ — できないものを、
/// できるように見せない(`cube` や `heart` を並べれば、四角が置かれる)。
/// 組は (鍵=日本語, 見出し)。**引き当ては鍵**で、見出しだけが画面の言語になる。
pub(crate) fn shape_gallery(cat: &str) -> Vec<(&'static str, &'static str)> {
    match cat {
        "基本図形" => vec![
            ui::item!("四角形"),
            ui::item!("角丸四角形"),
            ui::item!("楕円"),
            ui::item!("三角形"),
            ui::item!("直角三角形"),
            ui::item!("平行四辺形"),
            ui::item!("台形"),
            ui::item!("ひし形"),
            ui::item!("五角形"),
            ui::item!("六角形"),
            ui::item!("八角形"),
            ui::item!("十字"),
        ],
        "ブロック矢印" => vec![
            ui::item!("右矢印"),
            ui::item!("左矢印"),
            ui::item!("上矢印"),
            ui::item!("下矢印"),
            ui::item!("左右矢印"),
            ui::item!("上下矢印"),
        ],
        "数式図形" => vec![
            ui::item!("加算記号"),
            ui::item!("減算記号"),
            ui::item!("乗算記号"),
            ui::item!("等号"),
            ui::item!("不等号"),
        ],
        "フローチャート" => vec![
            ui::item!("処理"),
            ui::item!("判断"),
            ui::item!("データ"),
            ui::item!("端子"),
            ui::item!("書類"),
            ui::item!("結合子"),
        ],
        "星とリボン" => vec![
            ui::item!("星 4"),
            ui::item!("星 5"),
            ui::item!("星 6"),
            ui::item!("星 8"),
        ],
        "吹き出し" => vec![
            ui::item!("四角形の吹き出し"),
            ui::item!("円形の吹き出し"),
        ],
        "線" => vec![ui::item!("直線"), ui::item!("自由な形(点で作る)")],
        _ => Vec::new(),
    }
}

/// 分類の鍵 → 画面の見出し(2段目の題に出す)
pub(crate) fn shape_cat_label(cat: &str) -> &'static str {
    match cat {
        "ブロック矢印" => ui::t!("ブロック矢印"),
        "数式図形" => ui::t!("数式図形"),
        "フローチャート" => ui::t!("フローチャート"),
        "星とリボン" => ui::t!("星とリボン"),
        "吹き出し" => ui::t!("吹き出し"),
        "線" => ui::t!("線"),
        _ => ui::t!("基本図形"),
    }
}

/// 図形の鍵 → (prstGeom の名前, 画面の見出し)。
/// **知らない鍵は四角**(一覧から来る限り起こらないが、黙って落とさない)
pub(crate) fn shape_kind(v: &str) -> (&'static str, &'static str) {
    match v {
        "角丸四角形" => ("roundRect", ui::t!("角丸四角形")),
        "楕円" => ("ellipse", ui::t!("楕円")),
        "三角形" => ("triangle", ui::t!("三角形")),
        "直角三角形" => ("rtTriangle", ui::t!("直角三角形")),
        "平行四辺形" => ("parallelogram", ui::t!("平行四辺形")),
        "台形" => ("trapezoid", ui::t!("台形")),
        "ひし形" => ("diamond", ui::t!("ひし形")),
        "五角形" => ("pentagon", ui::t!("五角形")),
        "六角形" => ("hexagon", ui::t!("六角形")),
        "八角形" => ("octagon", ui::t!("八角形")),
        "十字" => ("plus", ui::t!("十字")),
        "右矢印" => ("rightArrow", ui::t!("右矢印")),
        "左矢印" => ("leftArrow", ui::t!("左矢印")),
        "上矢印" => ("upArrow", ui::t!("上矢印")),
        "下矢印" => ("downArrow", ui::t!("下矢印")),
        "左右矢印" => ("leftRightArrow", ui::t!("左右矢印")),
        "上下矢印" => ("upDownArrow", ui::t!("上下矢印")),
        "加算記号" => ("mathPlus", ui::t!("加算記号")),
        "減算記号" => ("mathMinus", ui::t!("減算記号")),
        "乗算記号" => ("mathMultiply", ui::t!("乗算記号")),
        "等号" => ("mathEqual", ui::t!("等号")),
        "不等号" => ("mathNotEqual", ui::t!("不等号")),
        "処理" => ("flowChartProcess", ui::t!("処理")),
        "判断" => ("flowChartDecision", ui::t!("判断")),
        "データ" => ("flowChartInputOutput", ui::t!("データ")),
        "端子" => ("flowChartTerminator", ui::t!("端子")),
        "書類" => ("flowChartDocument", ui::t!("書類")),
        "結合子" => ("flowChartConnector", ui::t!("結合子")),
        "星 4" => ("star4", ui::t!("星 4")),
        "星 5" => ("star5", ui::t!("星 5")),
        "星 6" => ("star6", ui::t!("星 6")),
        "星 8" => ("star8", ui::t!("星 8")),
        "四角形の吹き出し" => ("wedgeRectCallout", ui::t!("四角形の吹き出し")),
        "円形の吹き出し" => ("wedgeEllipseCallout", ui::t!("円形の吹き出し")),
        "直線" => ("line", ui::t!("直線")),
        "自由な形(点で作る)" => ("path", ui::t!("自由な形(点で作る)")),
        _ => ("rect", ui::t!("四角形")),
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
        ui::item!("網 6.25%"),
        ui::item!("網 12.5%"),
        ui::item!("網 25%"),
        ui::item!("網 50%"),
        ui::item!("斜線"),
        ui::item!("市松"),
    ]
}

/// 柄の鍵 → xlsx の patternType
pub(crate) fn pattern_kind(v: &str) -> Option<&'static str> {
    Some(match v {
        "網 6.25%" => "gray0625",
        "網 12.5%" => "gray125",
        "網 25%" => "lightGray",
        "網 50%" => "mediumGray",
        "斜線" => "darkUp",
        "市松" => "darkGrid",
        _ => return None,
    })
}

/// グラデーションの向き。**角度で持つ**(xlsx の degree)
pub(crate) fn grad_dirs() -> Vec<(&'static str, &'static str)> {
    vec![
        ui::item!("横(左から右)"),
        ui::item!("縦(上から下)"),
        ui::item!("斜め(左上から右下)"),
        ui::item!("斜め(左下から右上)"),
    ]
}

/// 向きの鍵 → (角度×100, 放射か)。
/// **放射は並べない** — GPUI に放射の背景が無く、線形で代用すると
/// 選んだ物と見える物が食い違う。読みは受ける(往復は保つ)
pub(crate) fn grad_dir_of(v: &str) -> Option<(i32, bool)> {
    Some(match v {
        "横(左から右)" => (0, false),
        "縦(上から下)" => (9000, false),
        "斜め(左上から右下)" => (4500, false),
        "斜め(左下から右上)" => (31500, false),
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
    f: &sheet::model::CellFormat,
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
