#!/usr/bin/env python3
"""テンプレート(`.tmpl.adoc`)の言葉の表を作る。

    python3 ui/gen_tmpl_words.py           # 生成して、訳の穴を報告する
    python3 ui/gen_tmpl_words.py --check   # 生成せずに穴だけ数える

## なぜ engine の中に表を置くのか

テンプレートの読み書きは `kumihan`(adoc のエンジン)の持ち場です。
`kumihan` は `lang` に依存しません — 組版のエンジンが言語の表や Python の
実行機構を引きずらないためです。そこで、**訳の出どころは `ui/i18n` の1つ
のまま**にして、そこから `engine/src/booktmpl/words.rs` を起こします。

## 鍵は記号

表の題も列の見出しも値の語も、記号で持ちます。**書くときは画面の言語、
読むときはどの言語でも受ける**ためです。配られたテンプレートを別の国の人が
開いても読めないと困ります。

既にある鍵は使い回します(`太字` は `bold`、`用紙` は `paper`)。
リボンや小窓の文言と同じ語なので、訳を2箇所に持ちません。
"""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
I18N = HERE / "i18n"
OUT = ROOT / "engine/src/booktmpl/words.rs"

# **テンプレートが使う言葉。**(記号, 日本語, 英語)
#
# 日本語と英語はここに書きます。他の 13 言語は `ui/i18n/<loc>.json` から
# 引き、無ければ英語に落ちます(落ちた分はこの道具が数えて言います)。
WORDS = [
    # --- 表の題 ---
    ("paper",              "用紙",           "Paper"),
    ("col_width",          "列幅",           "Column width"),
    ("row_height",         "行の高さ",       "Row height"),
    ("print",              "印刷",           "Print"),
    ("page_break",         "改ページ",       "Page break"),
    ("header_footer",      "ヘッダーとフッター", "Header and footer"),
    ("view",               "画面",           "View"),
    ("tmpl_group",         "グループ化",     "Group"),
    ("tmpl_protect",       "保護",           "Protection"),
    ("format",             "書式",           "Format"),
    ("format_applied",     "書式の当て",     "Format applied to"),
    ("workbook",           "ブック",         "Workbook"),
    # --- 列の見出し ---
    ("sheets",             "シート",         "Sheet"),
    ("tmpl_column",           "列",             "Column"),
    ("width_2",            "幅",             "Width"),
    ("row",                "行",             "Row"),
    ("height",             "高さ",           "Height"),
    ("size",               "大きさ",         "Size"),
    ("orientation",        "向き",           "Orientation"),
    ("margins",            "余白",           "Margins"),
    ("gridlines",          "目盛線",         "Gridlines"),
    ("tmpl_zoom",             "拡大",           "Zoom"),
    ("scale",              "倍率",           "Scale"),
    # **紙に収める枚数は「横,縦」の1欄**にしました(2026-08-28)。
    # 下の2つは古いテンプレートを読むために残してあります
    ("fit_to_page",        "紙に収める",     "Fit to page"),
    ("fit_to_width",       "横に収める",     "Fit to width"),
    ("fit_to_height",      "縦に収める",     "Fit to height"),
    ("row_col_headings",   "行列番号",       "Row and column headings"),
    ("title_rows",         "タイトル行",     "Title rows"),
    ("title_cols",         "タイトル列",     "Title columns"),
    ("position",           "位置",           "Position"),
    ("tmpl_text",          "文字",           "Text"),
    ("freeze",             "固定",           "Freeze"),
    ("formula_2",          "数式",           "Formula"),
    ("rtl",                "右横書き",       "Right to left"),
    ("hide",               "非表示",         "Hide"),
    ("tab_color",          "見出しの色",     "Tab color"),
    # **既定の幅と高さは、列幅と行の高さの表に「既定」の行として置きます**
    # (2026-08-28)。「既定の列幅」を1語で持つと訳が引けません。
    # 下の2つは古いテンプレートを読むために残してあります
    ("default_2",          "既定",           "Default"),
    ("default_col_width",  "既定の列幅",     "Default column width"),
    ("default_row_height", "既定の行の高さ", "Default row height"),
    ("kind",               "種類",           "Kind"),
    ("level",              "段",             "Level"),
    ("tmpl_collapsed",     "畳む",           "Collapsed"),
    ("allowed_actions",    "許可する操作",   "Allowed actions"),
    ("name",               "名前",           "Name"),
    ("item",               "項目",           "Item"),
    ("value",              "値",             "Value"),
    ("range",              "範囲",           "Range"),
    # --- 値の語 ---
    ("landscape_2",        "横",             "Landscape"),
    ("portrait",           "縦",             "Portrait"),
    ("header",             "ヘッダー",       "Header"),
    ("footer",             "フッター",       "Footer"),
    # **ヘッダーの「対象」は別の列にします**(2026-08-28)。「偶数ヘッダー」
    # のような複合語は、どの製品も1語では持たないので訳が引けません。
    # 「偶数の頁」「先頭の頁」なら本家がどの言語でも持っています。
    # 下の4つは**古いテンプレートを読むため**に残してあります
    ("even_page",          "偶数の頁",       "Even page"),
    ("first_page",         "先頭の頁",       "First page"),
    ("all_pages",          "すべて",         "All"),
    ("header_even",        "偶数ヘッダー",   "Even page header"),
    ("footer_even",        "偶数フッター",   "Even page footer"),
    ("header_first",       "先頭ヘッダー",   "First page header"),
    ("footer_first",       "先頭フッター",   "First page footer"),
    ("theme_colors",       "テーマ色",       "Theme colors"),
    ("show_r1c1",          "R1C1 で見せる",  "Show as R1C1"),
    # --- 保護中も許す操作 ---
    ("select_locked_cells",   "ロックされたセルの選択",     "Select locked cells"),
    ("select_unlocked_cells", "ロックされていないセルの選択", "Select unlocked cells"),
    ("format_cells",       "セルの書式設定", "Format cells"),
    ("format_columns",     "列の書式設定",   "Format columns"),
    ("format_rows",        "行の書式設定",   "Format rows"),
    ("insert_columns",     "列の挿入",       "Insert columns"),
    ("insert_rows",        "行の挿入",       "Insert rows"),
    ("insert_hyperlinks",  "ハイパーリンクの挿入", "Insert hyperlinks"),
    ("delete_columns",     "列の削除",       "Delete columns"),
    ("delete_rows",        "行の削除",       "Delete rows"),
    ("sort_2",             "並べ替え",       "Sort"),
    ("use_autofilter",     "オートフィルターの使用", "Use AutoFilter"),
    ("use_pivottable",     "ピボットテーブルの使用", "Use PivotTable reports"),
    ("edit_objects",       "オブジェクトの編集", "Edit objects"),
    # --- 書式の項目 ---
    ("bold",               "太字",           "Bold"),
    ("italic",             "斜体",           "Italic"),
    ("underline",          "下線",           "Underline"),
    ("strikethrough",      "取り消し線",     "Strikethrough"),
    ("subscript",          "下付き",         "Subscript"),
    ("tmpl_borders",       "罫線",           "Borders"),
    ("halign",             "横位置",         "Horizontal alignment"),
    ("valign",             "縦位置",         "Vertical alignment"),
    ("fill_color",         "塗り",           "Fill color"),
    ("fill_bg",            "塗りの地",       "Fill background"),
    ("fill_pattern",       "塗りの柄",       "Fill pattern"),
    ("gradient_2",         "グラデーション", "Gradient"),
    ("fill_theme",         "塗りのテーマ色", "Fill theme color"),
    ("font_color",         "文字色",         "Font color"),
    ("color_theme",        "文字のテーマ色", "Font theme color"),
    ("tmpl_font",             "書体",           "Font"),
    ("rotation_2",         "回転",           "Rotation"),
    ("wrap",               "折り返して全体を表示", "Wrap text"),
    ("shrink",             "縮小",           "Shrink to fit"),
    ("indent_3",           "字下げ",         "Indent"),
    ("number_format",      "表示形式",       "Number format"),
    ("unlocked",           "ロック解除",     "Unlocked"),
    ("hide_formula",       "式を隠す",       "Hidden formula"),
    # --- 罫線の線種 ---
    ("hairline",           "極細",           "Hair"),
    ("dotted",             "点線",           "Dotted"),
    ("dash_dot_dot",       "二点鎖線",       "Dash dot dot"),
    ("dash_dot",           "一点鎖線",       "Dash dot"),
    ("dashed",             "破線",           "Dashed"),
    ("thin",               "細",             "Thin"),
    ("medium_dash_dot_dot", "中太の二点鎖線", "Medium dash dot dot"),
    ("medium_dash_dot",    "中太の一点鎖線", "Medium dash dot"),
    ("medium_dashed",      "中太の破線",     "Medium dashed"),
    ("medium",             "中",             "Medium"),
    ("thick",              "太",             "Thick"),
    ("double",             "二重",           "Double"),
    ("slant_dash_dot",     "斜め一点鎖線",   "Slanted dash dot"),
    # --- 横の位置 ---
    ("align_general",      "標準",           "General"),
    ("left",               "左",             "Left"),
    ("center",             "中央",           "Center"),
    ("right",              "右",             "Right"),
    ("justify",            "両端揃え",       "Justify"),
    ("center_across",      "選択範囲内で中央", "Center across selection"),
    ("distributed",        "均等割付",       "Distributed"),
    # --- 縦の位置 ---
    ("top",                "上",             "Top"),
    ("bottom",             "下",             "Bottom"),
    # --- 罫線の場所 ---
    ("edge_top",           "上辺",           "Top edge"),
    ("edge_bottom",        "下辺",           "Bottom edge"),
    ("edge_left",          "左辺",           "Left edge"),
    ("edge_right",         "右辺",           "Right edge"),
]

LANGS = sorted(p.stem for p in I18N.glob("*.json") if p.stem != "keys")


def load(loc):
    return json.loads((I18N / f"{loc}.json").read_text(encoding="utf-8"))


def esc(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def main():
    check = "--check" in sys.argv
    tables = {l: load(l) for l in LANGS}
    ja, en = {k: j for k, j, _ in WORDS}, {k: e for k, _, e in WORDS}

    # 記号の重なりを見る。**同じ記号を2度書くと、後の物が前を消します**
    seen = set()
    for k, _, _ in WORDS:
        if k in seen:
            sys.exit(f"記号が2度出ています: {k}")
        seen.add(k)

    # **使い回す記号は、日本語が同じであること。**
    #
    # `column_2` は既に「縦棒(カラム)」— グラフの種類 — で使われていました。
    # 記号だけ見て使い回すと、ドイツ語で `Säule`(建物の柱)が列の見出しに
    # 出ます(2026-08-26 に実際に出た)。**日本語を突き合わせれば機械で
    # 見つかります。**
    ちがい = [
        (k, j, tables["ja"][k])
        for k, j, _ in WORDS
        if k in tables["ja"] and tables["ja"][k] != j
    ]
    if ちがい:
        print("**記号の指す物が食い違っています。** 別の記号にしてください:")
        for k, mine, theirs in ちがい:
            print(f"    {k:22} こちら {mine!r} ↔ ui/i18n {theirs!r}")
        sys.exit(1)

    # **穴埋めの入った訳は使えません。** 文言の鍵と記号がぶつかると、
    # `Zoom {}%` のような文がテンプレートの見出しに出ます(2026-08-26 に
    # 実際に出た)。テンプレートの語は穴の無い名詞でなければなりません
    for k, _, _ in WORDS:
        for l in LANGS:
            t = tables[l].get(k, "")
            if "{" in t:
                sys.exit(f"記号 {k} の {l} の訳に穴埋めがあります: {t!r}\n"
                         f"    文言の鍵とぶつかっています。別の記号にしてください")

    holes = {}
    rows = []
    for k, _, _ in WORDS:
        cells = []
        for l in LANGS:
            t = tables[l].get(k, "").strip()
            if not t:
                t = {"ja": ja[k], "en": en[k]}.get(l, "")
            if not t:
                holes.setdefault(l, []).append(k)
                t = en[k]
            cells.append(t)
        rows.append((k, cells))

    body = ['//! **テンプレートの言葉の表(15言語)。**',
            '//!',
            '//! *この表は `python3 ui/gen_tmpl_words.py` が起こします。手で書かないでください。*',
            '//!',
            '//! 訳の出どころは `ui/i18n/<loc>.json` の1つです。`kumihan` は `lang` に',
            '//! 依存しないので(組版のエンジンが言語の表を引きずらないため)、',
            '//! ここへ写して持ちます。',
            '//!',
            '//! **書くときは画面の言語、読むときはどの言語でも受けます。** 配られた',
            '//! テンプレートを別の国の人が開いても読めないと困るからです。',
            '',
            'use crate::font::default_language;',
            '',
            '/// 表が持っている言語の札(並びは下の表の桁と同じ)',
            'pub const LANGS: &[&str] = &[' + ", ".join(f'"{l}"' for l in LANGS) + '];',
            '',
            '/// (記号, 言語ごとの訳)。桁の並びは [`LANGS`] と同じです',
            f'pub const WORDS: &[(&str, [&str; {len(LANGS)}])] = &[']
    for k, cells in rows:
        body.append(f'    ("{k}", [' + ", ".join(f'"{esc(c)}"' for c in cells) + ']),')
    body += [
        '];',
        '',
        '/// いまの画面の言語の桁。知らない札は英語の桁',
        'fn column() -> usize {',
        '    let want = default_language();',
        '    LANGS.iter().position(|l| *l == want).unwrap_or_else(en_column)',
        '}',
        '',
        'fn en_column() -> usize {',
        '    LANGS.iter().position(|l| *l == "en").unwrap_or(0)',
        '}',
        '',
        '/// **記号 → いまの画面の言語の字。** 知らない記号は記号のまま返します',
        '/// (黙って空にしない — テンプレートに空の見出しが並ぶと読めません)。',
        'pub fn text(sym: &str) -> &\'static str {',
        '    match WORDS.iter().find(|(k, _)| *k == sym) {',
        '        Some((_, t)) => t[column()],',
        '        None => Box::leak(sym.to_string().into_boxed_str()),',
        '    }',
        '}',
        '',
        '/// **その字がこの記号を指しているか。どの言語でも受けます。**',
        '///',
        '/// 大文字小文字と前後の空白は見ません。',
        'pub fn is(sym: &str, text: &str) -> bool {',
        '    let t = text.trim();',
        '    WORDS',
        '        .iter()',
        '        .find(|(k, _)| *k == sym)',
        '        .is_some_and(|(_, v)| v.iter().any(|x| x.eq_ignore_ascii_case(t) || *x == t))',
        '}',
        '',
        '/// **並びの中から、その字が指す記号を選ぶ。**',
        '///',
        '/// 同じ字が別の記号を指すことがあります(英語の `Center` は横位置にも',
        '/// 縦位置にもある)。呼ぶ側が「この場所に来るのはこの記号のどれか」を',
        '/// 渡すことで取り違えを防ぎます。',
        '///',
        '/// **画面の言語を先に見ます。** 同じ字が言語をまたいで別の意味を',
        '/// 持つことがあるためです。台湾の中国語では `列` が行のことで、',
        '/// 日本語の `列` は列のことです。どの言語でも受ける作りのままだと、',
        '/// 日本語で書いたテンプレートの `列` が行として読まれます',
        '/// (2026-08-27 に、13 言語の訳を入れて往復の試験が落ちて分かりました)。',
        '/// 画面の言語で当たらなかったときだけ、他の言語も見ます。',
        'pub fn which(syms: &[&\'static str], text: &str) -> Option<&\'static str> {',
        '    let t = text.trim();',
        '    let c = column();',
        '    let ima = |s: &&str| {',
        '        WORDS',
        '            .iter()',
        '            .find(|(k, _)| k == s)',
        '            .is_some_and(|(_, v)| v[c].eq_ignore_ascii_case(t) || v[c] == t)',
        '    };',
        '    syms.iter().copied().find(|s| ima(s)).or_else(|| syms.iter().copied().find(|s| is(s, text)))',
        '}',
    ]
    if not check:
        OUT.write_text("\n".join(body) + "\n", encoding="utf-8")
        print(f"{OUT.relative_to(ROOT)} に {len(rows)} 語 × {len(LANGS)} 言語を書きました")

    新しい = [k for k, _, _ in WORDS if k not in tables["ja"]]
    if 新しい:
        print(f"**ui/i18n に無い記号 {len(新しい)} 個** — 足すまで英語のままです:")
        for k in 新しい:
            print(f"    {k:24} {ja[k]}")
    if holes:
        n = sum(len(v) for v in holes.values())
        print(f"訳の穴 {n} 件(英語に落ちています):")
        for l in sorted(holes):
            print(f"    {l}: {len(holes[l])} 語")
    if not 新しい and not holes:
        print("訳の穴はありません。")
    return 0


sys.exit(main())
