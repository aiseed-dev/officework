#!/usr/bin/env python3
"""メニュー・API・python-docx・openpyxl の対応表を起こす。

発注者 2026-08-24「メニュー、API、python-docx、openpyxl の関係をまとめた
一覧表を作れ」。

1行が1つの操作です。画面のボタン、`officework` の呼び方、そして本家
(python-docx / openpyxl)の呼び方が横に並びます。

*引くための表なので、独立した1枚に置きます*(2026-08-24 発注者
「これが、インデックスの一つになるから、独立させないとダメでしょう」)。
手引きの中に埋めると、引きたい人が手引きを読む羽目になります。

    python3 tools/api_taiou.py           # 揃っているか見る(CI の検査)
    python3 tools/api_taiou.py --write   # 手引きの節を書き直す

対応は下の表が持ちます。**本家の側は実際に呼んで確かめた名前**です
(python-docx 1.2.0 / openpyxl 3.1.5)。無い所は空です。
"""
import os
import re
import sys as _sys
_sys.path.insert(0, str(__import__('pathlib').Path(__file__).resolve().parent))
import i18n_ja  # noqa: E402  英語の鍵 → 日本語の札
import sys
from pathlib import Path
from typing import NamedTuple

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).parent))
import ribbon_parse  # noqa: E402

# **日本語のリボンを読みます**(2026-08-30)。
# 土台の `face/src/ribbon.rs` は 2026-08-26 の段2で札が英語になりました。
# そのまま読むと、日本語の手引きなのに段もボタンも英語で出ます。しかも
# 検査は落ちません(生成する道具なので、黙って英語の表を書きます)。
# id・並び・絵は ja 版でも同じです(`ribbon_ja.rs` の頭に書いてあります)。
RIBBON_JA = ROOT / "face/src/ribbon_ja.rs"

# 1行の中身。**ボタンの名前は英語と日本語の2つ**です(2026-08-30 発注者
# 「オブジェクトを廃止して、ボタンの項目に英語名と日本語名の2列にして」)。
#
# オブジェクトの列は廃止しました。`p.align` の `p` が段落だと右の列が
# 言っているので、203 行のうち 125 行は同じことを2度書いていました。
class Row(NamedTuple):
    tab: str        # 段(ホーム・挿入…)
    en: str         # 画面の英語の名前
    ja: str         # 画面の日本語の名前
    icon: str       # 絵の名前(無ければ空)
    mark: str       # 印(✅ ✍ ❌ 空)
    ow: str         # officework の書き方
    pd: str         # python-docx の書き方
    op: str         # openpyxl の書き方


# ボタンの id → (officework, python-docx, openpyxl)。
# **officework は `.adoc` を触る1つの模型なので、文書と表で列を割りません**
# (2026-08-24 発注者)。
# `A / B` は、いま2つの呼び方がある物です(寄せる仕事が残っています)。
MICHI = {
    # 描画 — 手書き。**docx だけの機能**で、adoc には居場所がありません
    "draw-select": ('', '', ''),
    "pen": ('', '', ''),
    "highlighter": ('', '', ''),
    "eraser": ('', '', ''),
    # マクロ — Python を動かす側。**プログラムからは自分で書けば済みます**
    "py-list": ('', '', ''),
    "py-new": ('', '', ''),
    "py-folder": ('', '', ''),
    "ribbon-list": ('', '', ''),
    "rec-toggle": ('', '', ''),
    "ai-macro": ('', '', ''),
    # 表のデザイン — テーブルの見た目。openpyxl は tables で持ちます
    "td-header": ('', '', 'ws.tables[…].tableStyleInfo'),
    "td-total": ('', '', 'ws.tables[…].totalsRowCount'),
    "td-band-row": ('', '', 'ws.tables[…].tableStyleInfo'),
    "td-band-col": ('', '', 'ws.tables[…].tableStyleInfo'),
    "td-first": ('', '', 'ws.tables[…].tableStyleInfo'),
    "td-last": ('', '', 'ws.tables[…].tableStyleInfo'),
    "td-filter": ('', '', 'ws.auto_filter'),
    "td-torange": ('', '', 'del ws.tables[…]'),
    "td-resize": ('', '', 'ws.tables[…].ref'),

    # **ダイアログのパラメータの台帳にあるのに、表に載っていなかった物**
    # (2026-08-25。docs/sekkei/dialog-parameters.ja.adoc から)。
    # 手引きにパラメータを書くには、まず表に行が要ります
    "table-tpl": ('', '', 'ws.add_table(…)'),
    "fit-pages": ('', '', 'ws.page_setup.fitToWidth'),
    "prot-allow": ('', '', 'ws.protection'),
    "scenario": ('', '', ''),
    "solver": ('', '', ''),
    "inssparkline": ('', '', ''),
    "insslicer": ('', '', ''),
    "paste-name": ('', '', 'wb.defined_names'),
    "watermark": ('', '', ''),
    "co-history": ('', '', ''),
    "changecase": ('', '', ''),
    "inssymbol": ('', '', ''),
    "datetime": ('', '', ''),
    "selectall": ('', '', ''),
    "text-from-file": ('', '', ''),
    "rem-duplicates": ('', '', ''),
    "flash-fill": ('', '', ''),
    "text-column": ('', '', ''),
    "subtotal": ('', '', ''),
    "trace-prec": ('', '', ''),
    "show-formulas": ('', '', ''),
    "fill-num": ('', '', ''),
    "numpages": ('', '', ''),
    "pagenum": ('', '', ''),
    "insrecommend": ('', '', ''),
    "func-list": ('', '', ''),
    "csv-kind": ('', '', ''),
    "data-from-text": ('', '', ''),
    "open": ('Doc.open(径路) / Book.open(径路)', 'docx.Document(径路)', 'load_workbook(径路)'),
    "save": ('d.save(径路) / b.save(径路)', 'd.save(径路)', 'wb.save(径路)'),
    "pdf": ('', '', ''),
    "copy": ("p.text = 値 / s['A1'] = 値 / c.text = 値", 'p.text = 値', "ws['A1'] = 値"),
    "cut": ("p.text = 値 / s['A1'] = 値 / c.text = 値", 'p.text = 値', "ws['A1'] = 値"),
    "paste": ("p.text = 値 / s['A1'] = 値 / c.text = 値", 'p.text = 値', "ws['A1'] = 値"),
    "clear": ("r.clear() / s['A1'] = None", 'r.clear()', "ws['A1'] = None"),
    "bold": ('r.bold / c.font', 'r.bold', 'c.font = Font(bold=True)'),
    "italic": ('r.italic / c.font', 'r.italic', 'c.font = Font(italic=True)'),
    "underline": ('r.underline / c.font', 'r.underline', 'c.font = Font(underline=…)'),
    "strikeout": ('r.strike', 'r.font.strike', ''),
    "fontname": ('r.font / c.font', 'r.font.name', 'c.font = Font(name=…)'),
    "fontsize": ('r.size_pt / c.font', 'r.font.size', 'c.font = Font(size=…)'),
    "incfont": ('r.size_pt / c.font', 'r.font.size', 'c.font = Font(size=…)'),
    "decfont": ('r.size_pt / c.font', 'r.font.size', 'c.font = Font(size=…)'),
    "fontcolor": ('r.color / c.font', 'r.font.color.rgb', 'c.font = Font(color=…)'),
    "superscript": ('', 'r.font.superscript', ''),
    "subscript": ('', 'r.font.subscript', 'c.font = Font(vertAlign=…)'),
    "clearstyle": ('r.clear()', 'r.clear()', ''),
    "ruby": ('', '', ''),
    "fillparag": ('c.fill', '', 'c.fill = PatternFill(…)'),
    # **文書とセルで掛かる相手が違います。** writer は段落を枠で囲み
    # (p.boxed)、calc はセルに線を引きます。段落の枠にはまだ呼び方が
    # ありません(2026-08-25 本家のマニュアルと突き合わせて分かった)
    "borders": ('c.border', '', 'c.border = Border(…)'),
    "align-left": ('p.align / c.alignment', 'p.alignment', 'c.alignment = Alignment(…)'),
    "align-center": ('p.align / c.alignment', 'p.alignment', 'c.alignment = Alignment(…)'),
    "align-right": ('p.align / c.alignment', 'p.alignment', 'c.alignment = Alignment(…)'),
    "align-just": ("p.align = 'justify'", 'p.alignment', ''),
    "align-dist": ("p.align = 'distribute'", '', ''),
    "wrap": ('c.alignment', '', 'c.alignment = Alignment(wrap_text=True)'),
    "merge": ('(col_span / v_merge) / s.merge_cells(…)', 'cell.merge(…)', "ws.merge_cells('A1:B2')"),
    "parastyle": ('p.style', 'p.style', ''),
    "markers": ("p.style = '箇条書き'", "p.style = 'List Bullet'", ''),
    "numbering": ("p.style = '番号付き'", "p.style = 'List Number'", ''),
    "multilevels": ('', '', ''),
    "decoffset": ('p.paragraph_format', 'p.paragraph_format.left_indent', ''),
    "incoffset": ('p.paragraph_format', 'p.paragraph_format.left_indent', ''),
    "linespace": ('p.paragraph_format.line_spacing', 'p.paragraph_format.line_spacing', ''),
    "replace": ('d.replace(前, 後)', '', ''),
    "format": ('c.number_format', '', 'c.number_format'),
    "currency": ('c.number_format', '', 'c.number_format'),
    "percents": ('c.number_format', '', 'c.number_format'),
    "comma": ('c.number_format', '', 'c.number_format'),
    "cell-ins": ('t.add_row() / s.insert_rows(行)', 't.add_row()', 'ws.insert_rows(行)'),
    "cell-del": ('s.delete_rows(行)', '', 'ws.delete_rows(行)'),
    "condformat": ('', '', 'ws.conditional_formatting.add(…)'),
    "sum": ("s['A1'] = '=SUM(…)'", '', "ws['A1'] = '=SUM(…)'"),
    "defname": ('b.create_named_range(名前, …)', '', 'wb.defined_names'),
    "sort-asc": ('', '', ''),
    "sort-desc": ('', '', ''),
    "setfilter": ('', '', 'ws.auto_filter.ref'),
    "clear-filter": ('', '', 'ws.auto_filter'),
    "instable": ('d.add_table(行, 列) / s.add_table(…)', 'd.add_table(行, 列)', 'ws.add_table(…)'),
    "insimage": ('d.add_picture(径路)', 'd.add_picture(径路)', 'ws.add_image(…)'),
    "inschart": ('', '', 'ws.add_chart(…)'),
    "blankpage": ('d.add_page_break()', 'd.add_page_break()', ''),
    "pagebreak": ('d.add_page_break()', 'd.add_page_break()', 'ws.row_breaks'),
    "edit-header": ('d.header / s.oddHeader', 'section.header', 'ws.oddHeader'),
    "edit-footer": ('d.footer', 'section.footer', ''),
    "controls": ('mcp.doc_fields()', '', ''),
    "insequation": ('', '', ''),
    "inshyperlink": ('c.hyperlink', '', 'c.hyperlink'),
    "pivot-insert": ('', '', 'ws.add_pivot(…)'),
    "pagemargins": ('d.sections[0]', 'section.left_margin', 'ws.page_margins'),
    "pageorient": ('d.sections[0]', 'section.orientation', 'ws.page_setup.orientation'),
    "pagesize": ('d.sections[0]', 'section.page_width', 'ws.page_setup.paperSize'),
    "printarea": ('s.print_area', '', 'ws.print_area'),
    "printtitles": ('s.print_title_rows', '', 'ws.print_title_rows'),
    "print-gridlines": ('s.print_gridlines', '', 'ws.print_options.gridLines'),
    "insert-function": ("s['A1'] = '=…'", '', "ws['A1'] = '=…'"),
    "calc-mode": ('b.recalc()', '', 'wb.calculation'),
    "data-validation": ('s.add_data_validation(…)', '', 'ws.add_data_validation(…)'),
    "group": ('s.row_groups', '', 'ws.column_dimensions[…].outline_level'),
    "ungroup": ('s.row_groups', '', 'ws.column_dimensions[…].outline_level'),
    "toc": ('', '', ''),
    "bookmarks": ('', '', ''),
    "crossref": ('', '', ''),
    "footnote": ('', '', ''),
    "caption": ('', '', ''),
    # 記入欄。**事務の様式の中心**なので、11 個のボタンを全部載せます
    # (2026-08-25 まで、テキストフィールドと名前の2つしか載っていませんでした)。
    # 種類は docx の w:sdt に往復します。値の出し入れは名前で引くので、
    # どの種類でも呼び方は同じです
    "form-text": ('mcp.doc_fill({member: 値})', '', ''),
    "form-name": ('mcp.doc_fields()', '', ''),
    "form-combo": ('mcp.doc_fill({member: 値})', '', ''),
    "form-dropdown": ('mcp.doc_fill({member: 値})', '', ''),
    "form-checkbox": ('mcp.doc_fill({member: 値})', '', ''),
    "form-radio": ('mcp.doc_fill({member: 値})', '', ''),
    "form-image": ('', '', ''),
    "form-email": ('mcp.doc_fill({member: 値})', '', ''),
    "form-phone": ('mcp.doc_fill({member: 値})', '', ''),
    "form-complex": ('mcp.doc_fill({member: 値})', '', ''),
    "form-signature": ('', '', ''),
    "co-addcomment": ('p.add_comment(文) / c.comment', 'p.add_comment(文)', 'c.comment = Comment(…)'),
    "co-showcomment": ('d.comments / c.comment', 'd.comments', 'c.comment'),
    "prot-doc": ('', '', 'ws.protection'),
    "prot-encrypt": ('', '', 'wb.security'),
    "freeze": ('s.freeze_panes', '', 'ws.freeze_panes'),
    "show-gridlines": ('s.show_gridlines', '', 'ws.sheet_view.showGridLines'),
    # **左右のパネルを出すボタン**(2026-08-30)。
    # `show-right` はファイルのページの表に入れてありましたが、あちらは
    # `f-` で始まる id しか読まないので、行が1つも出ていませんでした。
    # 表示の段のボタンなので、こちらが居場所です
    "show-left": ('', '', ''),
    "show-right": ('', '', ''),
}

# **実装しないと決めた物**(id → 理由)。
# *ここに載せるのは、決めが記録されている物だけ*です。
# 決めていない空欄は「未実装」— 作らないと決めたのではなく、まだ作っていません。
TSUKURANAI = {
    # 描画(手書き)。docx だけの機能で、adoc には居場所がありません
    # マクロ(Python を動かす側)
    # ファイルのページの、画面だけの物。**文書は変わりません**
}


# **書けば済む物**(id → 書き方)。専用の口は作りません。
#
# 発注者 2026-08-24「別にマクロ等で書けたらいいので、すべて操作できるように
# するのは難しくない」。*そのとおりで、いまある口を組み合わせれば書ける物が
# たくさんあります*。専用の口を足すより、書き方を1行見せるほうが早いのです。
KAKEBA = {
    "changecase": "r.text = r.text.upper()",
    "inssymbol": "r.text += '※'(字をそのまま打つ)",
    "datetime": "p.text = date.today().strftime('%Y年%m月%d日')",
    "selectall": "d.paragraphs(全部を順に回る)",
    "text-from-file": "d.add_paragraph(open('メモ.txt').read())",
    "rem-duplicates": "見た = set() で行を選り分ける",
    "flash-fill": "s['B2'] = s['A2'].split()[0](規則を書く)",
    "text-column": "s['B2'], s['C2'] = s['A2'].split(',')",
    "subtotal": "s['A9'] = '=SUBTOTAL(9,A2:A8)'",
    "trace-prec": "s.formula('A1') で参照を読む",
    "show-formulas": "s.formula(場所)",
    "fill-num": "for i in range(10): s[f'A{i+2}'] = i + 1",
    "numpages": "(ヘッダーの `##`)",
    "pagenum": "(ヘッダーの `#`)",
    "insrecommend": "s.values() を polars に渡して選ぶ",
    "func-list": "自分の .py を書く(綴りの macros)",
    "csv-kind": "csv モジュールで読む",
    "data-from-text": "csv モジュールで読んで s['A1'] へ",
}


# 画面の飾り。ボタンには付いていますが、*コマンドの名前ではありません*
KAZARI = re.compile(r"^[‹›<>]\s*|\s*[((][^))]*[))]\s*$")


def command_name(label: str) -> str:
    """画面のラベルから、コマンドの名前を取り出す。

    ファイル名も見出しも*これ*にします(2026-08-25 発注者「ファイル名や
    タイトルはコマンド名にする」)。画面の「‹ 戻る」は戻る、
    「データを差し込む(CSV)」はデータを差し込むです。
    括弧の中は形式の断り書きで、名前の一部ではありません。

    対応表のほうは*画面のまま*にします。あちらは画面を引くための表なので、
    押しているボタンの字がそのまま出ていないと引けません。

    **句点があるときは、その前までが名前です**(2026-08-30)。右パネルの
    見出しは「ページ設定。文書全体に掛かります」のように、名前と説明を
    並べて出しています。手引きの名前に要るのは前の方だけです。
    """
    return KAZARI.sub("", label.split("。")[0]).strip() or label


# ファイル名に使えない字。`/` は制約で置き換えるだけで、名前の一部です
FNAME_NG = re.compile(r'[/\\:*?"<>|]')
_manual_table = None


def manual_link(label: str) -> str:
    """ボタンの名前を、手引きへのリンクにして返す。

    **一覧から手引きへ飛べるようにします**(2026-08-25 発注者「一覧からの
    リンクをつける」)。この表は引くための1枚なので、行を引き当てた人が
    そのまま詳しい説明へ行けないと、そこで止まります。

    手引きがまだ無いボタンは、名前をそのまま返します(リンクにしません)。

    **径路はこの文書(`docs/ja/api-taiou.adoc`)から見た相対です。**
    `docs` から数えると `ja/commands/…` になり、`docs/ja/ja/commands/…` を
    指してリンクが全部切れます(2026-08-26 の言語別フォルダへの移動で
    こうなっていました)。
    """
    global _manual_table
    if _manual_table is None:
        _manual_table = {}
        ahead = ROOT / "docs/ja/commands"
        for q in ahead.rglob("*.adoc"):
            if q.name != "README.ja.adoc":
                _manual_table[q.stem] = q.relative_to(ROOT / "docs/ja").as_posix()
    name = FNAME_NG.sub("_", command_name(label)).strip()
    to = _manual_table.get(name)
    return f"link:{to}[{label}]" if to else label


def state(id_: str, ow: str) -> str:
    """印を返す(2026-08-24 発注者「実装できたら ✅、実装しないは ❌」)。

    印が言うのは*プログラムから呼べるか*だけです。ボタンのほうは、
    印が何であっても画面から使えます。

    * `✅` 実装した — 専用の呼び方があります
    * `✍` 書けば済む — 専用の呼び方は作りません。書き方をその行に出します
    * `❌` 呼ぶ相手が無い — *画面の見え方が変わるだけ*の操作です
    * *空* まだ — 呼べるようにしていない物。**ここが仕事の一覧**です

    **`❌` を付ける前に確かめること**(2026-08-25 発注者「フォルダから探すを
    ❌にしたらダメでしょう。api がないというだけでしょう」)。
    その操作で*文書かファイルが変わるなら* `❌` ではありません。
    API が無いだけなら*空*です。この間違いで、フォルダの検索・ピボット・
    最近開いた・描画など 23 件が「作らない」に見えていました。
    """
    if ow:
        return "✅"
    if id_ in TSUKURANAI:
        return "❌"
    if id_ in KAKEBA:
        return "✍"
    return ""


# アイコンの置き場(この文書から見た相対の径路)。
# **文書は `docs/ja/` にあります**(2026-08-26 に言語別のフォルダへ移りました)。
# ここが `../face/icons` のままだと `docs/face/icons` を指し、絵が1つも出ません
ICON_DIR = "../../face/icons"

ICONS_RS = ROOT / "face/src/icons.rs"


def _icon_file() -> dict:
    """**絵の名前 → ファイル名。** `icons.rs` が繋いでいる対応を読みます。

    名前とファイル名が違う物があります(`insertimage` の実体は
    `insimage.svg`)。画面は `icons.rs` を通るので出ますが、文書から
    直に指すと届きません — 2026-08-24 にこの表で1件踏みました。
    """
    out = {}
    try:
        src = ICONS_RS.read_text(encoding="utf-8")
    except OSError:
        return out
    for m in re.finditer(r'\("([a-z0-9-]+)",\s*include_bytes!\("\.\./icons/([^"]+)\.svg"\)\)', src):
        if m.group(1) != m.group(2):
            out[m.group(1)] = m.group(2)
    return out


ICON_FILE = _icon_file()

MARK_S = "// api:taiou:start"
MARK_E = "// api:taiou:end"
SAKI = ROOT / "docs/ja/api-taiou.adoc"


def tab_layout(tabs):
    """**揃えた並び**(`face::tabs::merged` と同じ規則)。
    文章を軸にして、表だけの段をレイアウトの後ろへ入れます。"""
    w = [t.name for t in tabs["WRITER"]]
    c = [t.name for t in tabs["CALC"]]
    out = list(w)
    at = out.index("レイアウト") + 1 if "レイアウト" in out else len(out)
    for n in c:
        if n not in w:
            out.insert(at, n)
            at += 1
    return out


# **リボンにもファイルのページにも無い操作**(2026-08-24 発注者
# 「左右のサイドバーやクイックアクセスツールバーにあるものも入れろ」)。
#
# クイックアクセスは窓の1段目、左右のパネルは表示タブから開きます。
# *どちらもリボンの表に載っていない*ので、読むだけでは表に出てきません。
#
# **名前は画面の鍵で書きます**(2026-08-30)。鍵を書けば英語と日本語の
# 両方が `ui/i18n` から出るので、画面と揃います。ここを手で書いていた
# あいだ、シート見出しの右クリックは7行と書いてありましたが、画面には
# 10項目あって、6つは名前も違っていました。
#
# 画面に文言が無いボタンだけ、`(英語, 日本語)` の組を手で書きます。
# 絵だけのボタンと、コードに日本語を直に書いてあるメニューです。
# 英語が無いものは英語の欄を空にします — 無い物を作りません。
#
# (段, 名前, officework, python-docx, openpyxl)
HOKA = [
    # クイックアクセスは絵だけのボタンです。名前はその命令の名前を借ります
    ("クイックアクセス", "save", "d.save(径路) / b.save(径路)", "d.save(径路)", "wb.save(径路)"),
    ("クイックアクセス", "print", "", "", ""),
    # 元に戻す・やり直しは、画面のどこにも文言がありません(絵だけで、
    # 命令の側にも訳がありません)。よく知られた名前を手で書きます
    ("クイックアクセス", ("Undo", "元に戻す"), "", "", ""),
    ("クイックアクセス", ("Redo", "やり直し"), "", "", ""),
    ("左パネル", "heading", "p.style で拾う", "p.style", ""),
    ("左パネル", "comment", "d.comments", "d.comments", "c.comment"),
    ("左パネル", "find", "d.find(字)", "", ""),
    ("左パネル", "ai", "", "", ""),
    # **右パネルは3つのボタンです**(`writer/src/panels.rs`)。
    # 「設定・ページ・スタイル」と1行にまとめてありましたが、3つ目は
    # スタイルではなくフォルダの中身でした
    ("右パネル", "settings_adjust_where_cursor", "", "", ""),
    ("右パネル", "page_settings_whole_document", "d.sections[0]", "section.page_width", "ws.page_setup.paperSize"),
    ("右パネル", "styles_edit_template", "p.style", "p.style", ""),
    ("右パネル", "files_what_folder", "", "", ""),
    # 窓の下端(ステータスバー)。数を入れる所は名前から外します
    ("下端", "page", "(紙に組んで数える)", "", ""),
    ("下端", "characters", "len(d.text)", "len(d.text)", ""),
    ("下端", "spell", "", "", ""),
    ("下端", "zoom", "", "", ""),
    # 形式と状態はコードに直に書いた字で、鍵がありません
    ("下端", ("", "ファイルの形式"), "", "", ""),
    ("下端", ("", "状態の文言"), "", "", ""),
    ("下端", ("", "選んだ範囲の合計・平均・個数"), "sum(…) / len(…)(値を読んで数える)", "", ""),
    # 右クリックのメニュー。**ほとんどがリボンと同じ命令**で、
    # ここにしか無い物だけを挙げます
    ("右クリック", "select_word", "", "", ""),
    ("右クリック", "select_line", "", "", ""),
    ("右クリック", "word_count", "len(d.text)", "len(d.text)", ""),
    # 読み飛ばした部品の一覧は officework だけの物です。
    # 本家の2つには同じ物がありません(2026-08-25 api_param_check が見つけた)
    ("右クリック", "skipped_version", "d.unsupported", "", ""),
    # 下の4つは `calc/src/view.rs` に日本語を直に書いてあるメニューです。
    # 英語の画面でも日本語のまま出るので、英語の名前がありません
    ("右クリック", ("", "形式を選択して貼り付け"), "", "", ""),
    ("右クリック", ("", "返信を追加"), "", "", ""),
    ("右クリック", ("", "マクロの割り当て"), "", "", ""),
    ("右クリック", ("", "画像として保存(SVG)"), "", "", ""),
    # シート見出しの右クリック(`calc/src/picks.rs` の 10 項目)
    ("シート見出しの右クリック", "insert", "b.add_sheet(名前)", "", "wb.create_sheet(名前)"),
    ("シート見出しの右クリック", "delete", "b.remove(名前)", "", "del wb[名前]"),
    ("シート見出しの右クリック", "rename", "s.title = 名前", "", "ws.title = 名前"),
    ("シート見出しの右クリック", "duplicate", "b.copy_worksheet(名前)", "", "wb.copy_worksheet(ws)"),
    ("シート見出しの右クリック", "move_left", "b.move_sheet(名前, 位置)", "", "wb.move_sheet(名前, 位置)"),
    ("シート見出しの右クリック", "move_right", "b.move_sheet(名前, 位置)", "", "wb.move_sheet(名前, 位置)"),
    ("シート見出しの右クリック", "hide", "", "", "ws.sheet_state"),
    ("シート見出しの右クリック", "unhide", "", "", "ws.sheet_state"),
    ("シート見出しの右クリック", "tab_colour", "", "", "ws.sheet_properties.tabColor"),
    ("シート見出しの右クリック", "protect_sheet", "s.protected = True", "", "ws.protection.sheet = True"),
]

# 文言の中の `{}`(数が入る所)を落として、名前だけにします。
# 画面は「{}/{} ページ」「ズーム {}%」と出しますが、表に要るのは
# 「ページ」「ズーム」です
_KAZU = re.compile(r"\{\}")


def _namae(text: str) -> str:
    # **短くしません。** 右パネルは「設定 — いる場所を直す」の形で、
    # ダッシュの前だけ取ると「設定」「ページ」「ファイル」になります。
    # 下端にも「ページ」があるので、別のボタンが同じ名前になって
    # 「同じものです」と嘘を書きました(2026-08-30 に実際に出た)
    return re.sub(r"\s+", " ", _KAZU.sub("", text)).strip(" /%,")


def hoka_name(spec) -> tuple:
    """HOKA の名前を (英語, 日本語) にします。

    鍵のときは `ui/i18n` から両方を引きます。組のときはそのまま返します。
    """
    if isinstance(spec, tuple):
        return spec
    return (_namae(i18n_ja.english(spec)), _namae(i18n_ja.japanese(spec)))

# 上の物のうち、専用の口を作らないと決めた物(理由つき)
# **リボンに無いボタンで、いま動くもの。**
# リボンのボタンは `face/src/ribbon.rs` の `ready` が状態を持ちますが、
# ファイルのページ・右クリック・シート見出しのボタンはそこに載りません。
# ここは*1つずつ実物を読んで確かめた*控えです(2026-08-25)。
# 手引きの状態(実装済み / 未実装)がこれで決まります。
HOKA_UGOKU = {
    # ファイルのページ — ui/src/filemenu.rs と writer/src/cmds.rs の分岐
    "‹ 戻る", "最近開いた", "フォルダから探す", "ファイルの場所を開く", "終了",
    # **フォルダーを開く**(2026-08-25 に足した口。`ui/src/filemenu.rs` の
    # `f-folder` → 各アプリの `folder_dialog_now`)。入れ忘れていたので、
    # 実装したのに手引きが「未実装」と出ていました
    "フォルダーを開く",
    "詳細設定",
    # **ヘルプ・機能のリクエスト・テンプレートから作成は灰色**なので
    # ここには入れません(`writer/src/cmds.rs` の `.grey()`)。
    # 入れていたせいで、手引きが「いま使えます」と嘘を書いていました
    "Web の形で書き出す(HTML)", "adoc 形式にする(本文と書式を分ける)", "保護する",
    # 右クリックとシート見出し
    "返信を追加",                       # calc/src/view.rs の comment-reply
    "画像として保存(SVG)",              # calc/src/view.rs の sh-save
    # **隠すと戻すは別のボタンです**(`calc/src/picks.rs` の hide / unhide)。
    # 「非表示・再表示」と1つに書いてありましたが、画面では2項目です
    "非表示", "再表示",
    "タブの色",
    # **パネルの面**(2026-08-30)。専用の呼び方はありませんが、画面では
    # どれも使えます。印だけで決めると「未実装」と出ます
    "設定", "プロジェクトパネル", "エージェントパネル",
}
# **マクロの割り当ては、まだ入っていません。** 図形やボタンにマクロを
# 結び付ける仕組みそのものがこれからです

HOKA_TSUKURANAI = {
    "語を選択": "画面の選択です。プログラムは字を直に切り出せます",
    "行を選択": "同じく画面の選択です",
    "形式を選択して貼り付け": "画面の貼り付けです。プログラムは入れる値を自分で選べます",
    "ファイルの形式": "画面の表示です。径路の拡張子を見れば分かります",
    "状態の文言": "画面の表示です。プログラムは返り値と unsupported を見ます",
    "スペル": "画面の表示の切り替えです",
    "ズーム": "画面の表示です。文書は変わりません",
    # **「AI と相談する」と「設定・ページ・スタイル」は消しました**
    # (2026-08-30)。前者はエージェントパネル、後者は4つのパネルに
    # 名前が変わりました。パネルは命令ではないので、ここには入れません
    "元に戻す": "画面の操作です。プログラムは保存する前の写しを持てます",
    "やり直し": "同じく画面の操作です",
}

FILE_SRC = ROOT / "writer/src/cmds.rs"

# ファイルのページの項目 → (オブジェクト, officework, python-docx, openpyxl)。
# **リボンのファイルタブは3つしかありません**(開く・保存・印刷)。
# 実体は全面のページで、`writer/src/cmds.rs` の `file_menu()` にあります。
# リボンだけを読むと、*ファイルの仕事がほとんど表に出ません*
# (2026-08-24 発注者「どうして対応表を変更しないのだ」)
FILE_MICHI = {
    "f-new": ('Doc() / Book()', 'docx.Document()', 'Workbook()'),
    "f-tpl": ('', 'docx.Document(雛形)', 'load_workbook(雛形)'),
    "f-open": ('Doc.open(径路) / Book.open(径路)', 'docx.Document(径路)', 'load_workbook(径路)'),
    # **フォルダを開き直す**(2026-08-25 発注者「どうしてフォルダーを開くが
    # ないのだ」)。綴りはフォルダなので、仕事を替えるとはフォルダを替えること。
    # プログラムは径路を直に書けるので、専用の呼び方は要りません
    "f-folder": ('', '', ''),
    # **形を選んで書き出す1つの入り口**。形ごとの呼び方は save に寄せます
    "f-export": ('d.save(径路) / b.save(径路)', 'd.save(径路)', 'wb.save(径路)'),
    "f-url": ('', '', ''),
    "f-recent": ('', '', ''),
    "f-find": ('', '', ''),
    "f-recover": ('', '', ''),
    "f-save": ('d.save(径路) / b.save(径路)', 'd.save(径路)', 'wb.save(径路)'),
    "f-saveas": ('d.save(別の径路)', 'd.save(別の径路)', 'wb.save(別の径路)'),
    "f-print": ('', '', ''),
    "f-merge": ('mcp.doc_merge_fields() / mcp.doc_fill(1行分)', '', ''),
    "f-html": ('', '', ''),
    "f-protect": ('', '', 'wb.security'),
    "f-distill": ('', '', ''),
    "f-info": ('d.core_properties', 'd.core_properties', 'wb.properties'),
    "f-place": ('', '', ''),
    "f-quit": ('', '', ''),
    "f-opts": ('', '', ''),
    "f-help": ('', '', ''),
    "f-req": ('', '', ''),
    "f-back": ('', '', ''),
}


def file_menu():
    """ファイルのページの項目を `writer/src/cmds.rs` から読みます。手で写しません。

    **鍵は英語なので、日本語に直してから返します**(2026-08-26 の移行)。
    この表は日本語のマニュアルで、手引きの頁の名前も日本語です。直さずに
    出すと、表だけ英語になって手引きへのリンクが全部切れます — しかも
    検査は落ちません(生成する道具なので、黙って英語の表を書きます)。
    """
    src = FILE_SRC.read_text(encoding="utf-8")
    body = src[src.index("fn file_menu"):]
    body = body[: body.index("\n    }")]
    out = re.findall(r'I::new\("(f-[a-z]+)",\s*ui::t!\("([^"]+)"\)\)', body)
    return [(i, i18n_ja.english(keys), i18n_ja.japanese(keys)) for i, keys in out]


def rows():
    """`Row` の並び。**並びはメニューのまま**です(2026-08-24 発注者)。

    ボタンの名前は英語と日本語の2つを持ちます。リボンは英語の
    `ribbon.rs` と日本語の `ribbon_ja.rs` を id で突き合わせます
    (2つのファイルは id も並びも同じで、`ribbon_locale_check` が見張ります)。
    """
    ja = ribbon_parse.tables_or_die(RIBBON_JA)
    en = ribbon_parse.tables_or_die(ribbon_parse.RIBBON)
    eigo = {c.id: c.label for app in en for t in en[app] for c in t.cmds if c.id}
    order = tab_layout(ja)
    w = {t.name: t for t in ja["WRITER"]}
    c = {t.name: t for t in ja["CALC"]}
    out = []
    for tab in order:
        if tab == "ファイル":
            # **リボンの3つではなく、全面のページの一覧を出します**
            for i, e, j in file_menu():
                if i not in FILE_MICHI:
                    continue
                ow, pd, op = FILE_MICHI[i]
                _label_lookup[i] = j
                out.append(Row(tab, e, j, "", state(i, ow), ow, pd, op))
            continue
        seen = set()
        for t in (w.get(tab), c.get(tab)):
            if t is None:
                continue
            for cmd in t.cmds:
                if not cmd.id or cmd.id in seen or cmd.id not in MICHI:
                    continue
                seen.add(cmd.id)
                ow, pd, op = MICHI[cmd.id]
                _label_lookup[cmd.id] = cmd.label
                out.append(Row(tab, eigo.get(cmd.id, ""), cmd.label, cmd.icon,
                               state(cmd.id, ow), ow, pd, op))
    for tab, spec, ow, pd, op in HOKA:
        e, j = hoka_name(spec)
        mark = "✅" if ow else ("❌" if j in HOKA_TSUKURANAI else "")
        _label_lookup[j] = j
        out.append(Row(tab, e, j, "", mark, ow, pd, op))
    return out


_label_lookup: dict = {}


def reason(label: str, st: str):
    """「実装しない」の理由。表の中で読めるようにします"""
    if st not in ("❌", "✍"):
        return None
    for table in (TSUKURANAI, KAKEBA, HOKA_TSUKURANAI):
        for k, v in table.items():
            if _label_lookup.get(k) == label:
                return v
    return None


def overlap(r):
    """**同じ操作が2か所以上に出ている物**を見つけます。

    発注者 2026-08-24「全部出したうえで、重複するものはそう書いておく」。
    *隠さずに出して、同じ物だと言う* — 画面には本当に両方あるからです。
    返りは (段, ボタン) → 最初に出た段。
    """
    first_seen, came_out = {}, {}
    for row in r:
        if row.ja in first_seen:
            came_out[(row.tab, row.ja)] = first_seen[row.ja]
        else:
            first_seen[row.ja] = row.tab
    return came_out


def table() -> str:
    r = rows()
    dup_of = overlap(r)
    o = []
    # **この節に説明を書きません。** 読み方はこの文書の頭にあります。
    # 利用者が読む物なので、作る側の話(生成の仕組み・作業の残り)は入れません
    o.append("")
    current = None
    for row in r:
        if row.tab != current:
            if current is not None:
                o.append("|===\n")
            # **見出しは `==`。** `===` にすると本家が「段が飛んでいる」と
            # 警告します(この節の前に `==` が無いため。2026-08-24 に実際に出た)
            o.append(f"== {row.tab}")
            o.append("")
            o.append('[cols="2,2,^1,3,3,3"]')
            o.append("|===")
            o.append("|英語の名前 |日本語の名前 |印 |officework |python-docx |openpyxl\n")
            current = row.tab
        f = lambda x: x if x else "—"
        inner = row.ow if row.ow else (reason(row.ja, row.mark) or "—")
        if (row.tab, row.ja) in dup_of:
            inner = (f"*{dup_of[(row.tab, row.ja)]}と同じ*"
                     + (f" — {inner}" if inner != "—" else ""))
        # **絵を名前の前に出します**(2026-08-24 発注者)。画面で見ている物と
        # 同じ絵なので、名前より先に目に入ります。径路は `face/icons` から
        # この文書の場所への相対です
        # **絵の名前とファイル名は、同じとは限りません。**
        # `face/src/icons.rs` が名前とファイルを繋いでいます(例: `insertimage`
        # の実体は `insimage.svg`)。画面はそちらを通るので出ますが、
        # 文書から直に指すと届きません。ここで解いてから書きます
        #
        # **絵の説明文は空にします**(2026-08-30)。名前がすぐ隣にあるので、
        # 入れると読み上げも本文の写しも名前が2回出ます
        name = ICON_FILE.get(row.icon, row.icon)
        icon_tag = f'image:{ICON_DIR}/{name}.svg[,16,16] ' if name else ""
        # **ボタンの名前から手引きへ飛ばします**(2026-08-25 発注者
        # 「一覧からのリンクをつける」)。この表は引くための1枚なので、
        # 引き当てた行からそのまま詳しい説明へ行けないと途中で止まります
        o.append(f"|{icon_tag}{f(row.en)} |{manual_link(row.ja)} |{row.mark} "
                 f"|{inner} |{f(row.pd)} |{f(row.op)}")
    if current is not None:
        o.append("|===\n")
    return "\n".join(o)


def cover():
    """**この表がどれだけ覆っているか**(2026-08-24)。

    「Python ですべて操作できる」と言うには、*表が全部のボタンを載せている*
    必要があります。載っていないボタンは、状態すら分かりません。
    """
    tabs = ribbon_parse.tables_or_die(RIBBON_JA)
    whole = {}
    for app in ("WRITER", "CALC"):
        for tab in tabs[app]:
            for c in tab.cmds:
                if c.id:
                    whole.setdefault(c.id, (tab.name, c.label))
    # **ファイルのページも数えます**(リボンのファイルタブは3つだけで、
    # 実際の仕事は全面のページにあります)
    for i, _e, j in file_menu():
        whole.setdefault(i, ("ファイル", j))
    # クイックアクセスと左右のパネル(リボンにもページにも無い物)
    hoka_ja = {hoka_name(spec)[1] for _tab, spec, *_ in HOKA}
    for tab, spec, *_ in HOKA:
        j = hoka_name(spec)[1]
        whole.setdefault(j, (tab, j))
    listed = [k for k in whole if k in MICHI or k in FILE_MICHI or k in hoka_ja]
    return len(listed), len(whole), sorted(
        (v[0], v[1], k) for k, v in whole.items()
        if k not in MICHI and k not in FILE_MICHI and k not in hoka_ja
    )


def main() -> int:
    src = SAKI.read_text(encoding="utf-8")
    m = re.search(rf"({re.escape(MARK_S)}[^\n]*\n)(.*?)(\n?{re.escape(MARK_E)})", src, re.S)
    if not m:
        print(f"::error::{SAKI.name} に {MARK_S} の印がありません", file=sys.stderr)
        return 1
    beki = table()
    if "--write" in sys.argv:
        SAKI.write_text(src[: m.start(2)] + beki + src[m.end(2):], encoding="utf-8")
        print(f"{SAKI.name} を書き直しました({len(rows())} 行)")
        return 0
    if m.group(2).strip() != beki.strip():
        # **手元では直します。落とすのは CI だけ**(2026-08-24 発注者
        # 「このような修正で検査が落ちないようにしろ」)。
        #
        # 生成物と道具がずれるのは、道具を直して `--write` を忘れたときです。
        # *それは機械が直せる*ので、手元では直して先へ進みます。
        # CI では直せません(直しても誰もコミットしない)ので、落として言います
        if os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS"):
            print(f"::error::{SAKI.name} の対応表が実物とずれています"
                  "(python3 tools/api_taiou.py --write で直してコミットしてください)",
                  file=sys.stderr)
            return 1
        SAKI.write_text(src[: m.start(2)] + beki + src[m.end(2):], encoding="utf-8")
        print(f"{SAKI.name} がずれていたので直しました({len(rows())} 行)。"
              "コミットに入れてください")
        return 0
    listed, whole, gaps = cover()
    if "--todo" in sys.argv:
        print(f"対応表に載っていないボタン {len(gaps)} 種:")
        for tab, l, i in gaps:
            print(f"  {tab:<12} {l:<24} {i}")
        return 0
    print(f"対応表は実物と揃っています({len(rows())} 行)。"
          f"押せるボタン {whole} 種のうち {listed} 種を載せています"
          f"(--todo で残りが出ます)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
