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

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).parent))
import ribbon_parse  # noqa: E402

# ボタンの id → (オブジェクト, officework, python-docx, openpyxl)。
# **officework は `.adoc` を触る1つの模型なので、文書と表で列を割りません**
# (2026-08-24 発注者)。どのオブジェクトの物かを示します。
# `A / B` は、いま2つの呼び方がある物です(寄せる仕事が残っています)。
MICHI = {
    # 描画 — 手書き。**docx だけの機能**で、adoc には居場所がありません
    "draw-select": ("", "", "", ""),
    "pen": ("", "", "", ""),
    "highlighter": ("", "", "", ""),
    "eraser": ("", "", "", ""),
    # マクロ — Python を動かす側。**プログラムからは自分で書けば済みます**
    "py-list": ("", "", "", ""),
    "py-new": ("", "", "", ""),
    "py-folder": ("", "", "", ""),
    "ribbon-list": ("", "", "", ""),
    "rec-toggle": ("", "", "", ""),
    "ai-macro": ("", "", "", ""),
    # 表のデザイン — テーブルの見た目。openpyxl は tables で持ちます
    "td-header": ("Table", "", "", "ws.tables[…].tableStyleInfo"),
    "td-total": ("Table", "", "", "ws.tables[…].totalsRowCount"),
    "td-band-row": ("Table", "", "", "ws.tables[…].tableStyleInfo"),
    "td-band-col": ("Table", "", "", "ws.tables[…].tableStyleInfo"),
    "td-first": ("Table", "", "", "ws.tables[…].tableStyleInfo"),
    "td-last": ("Table", "", "", "ws.tables[…].tableStyleInfo"),
    "td-filter": ("Table", "", "", "ws.auto_filter"),
    "td-torange": ("Table", "", "", "del ws.tables[…]"),
    "td-resize": ("Table", "", "", "ws.tables[…].ref"),

    # **ダイアログのパラメータの台帳にあるのに、表に載っていなかった物**
    # (2026-08-25。docs/sekkei/dialog-parameters.ja.adoc から)。
    # 手引きにパラメータを書くには、まず表に行が要ります
    "table-tpl": ("Sheet", "", "", "ws.add_table(…)"),
    "fit-pages": ("Sheet", "", "", "ws.page_setup.fitToWidth"),
    "prot-allow": ("Sheet", "", "", "ws.protection"),
    "scenario": ("Sheet", "", "", ""),
    "solver": ("Sheet", "", "", ""),
    "inssparkline": ("Sheet", "", "", ""),
    "insslicer": ("Sheet", "", "", ""),
    "paste-name": ("Book", "", "", "wb.defined_names"),
    "watermark": ("Doc", "", "", ""),
    "co-history": ("Doc", "", "", ""),
    "changecase": ("Run", "", "", ""),
    "inssymbol": ("Run", "", "", ""),
    "datetime": ("Paragraph", "", "", ""),
    "selectall": ("Doc", "", "", ""),
    "text-from-file": ("Doc", "", "", ""),
    "rem-duplicates": ("Sheet", "", "", ""),
    "flash-fill": ("Cell", "", "", ""),
    "text-column": ("Cell", "", "", ""),
    "subtotal": ("Cell", "", "", ""),
    "trace-prec": ("Cell", "", "", ""),
    "show-formulas": ("Cell", "", "", ""),
    "fill-num": ("Cell", "", "", ""),
    "numpages": ("Doc", "", "", ""),
    "pagenum": ("Doc", "", "", ""),
    "insrecommend": ("Sheet", "", "", ""),
    "func-list": ("Book", "", "", ""),
    "csv-kind": ("Sheet", "", "", ""),
    "data-from-text": ("Sheet", "", "", ""),
    "open": ("Doc / Book", "Doc.open(径路) / Book.open(径路)", "docx.Document(径路)", "load_workbook(径路)"),
    "save": ("Doc / Book", "d.save(径路) / b.save(径路)", "d.save(径路)", "wb.save(径路)"),
    "pdf": ("", "", "", ""),
    "copy": ("どこでも", "p.text = 値 / s['A1'] = 値 / c.text = 値", "p.text = 値", "ws['A1'] = 値"),
    "cut": ("どこでも", "p.text = 値 / s['A1'] = 値 / c.text = 値", "p.text = 値", "ws['A1'] = 値"),
    "paste": ("どこでも", "p.text = 値 / s['A1'] = 値 / c.text = 値", "p.text = 値", "ws['A1'] = 値"),
    "clear": ("Run / Cell", "r.clear() / s['A1'] = None", "r.clear()", "ws['A1'] = None"),
    "bold": ("Run / Cell", "r.bold / c.font", "r.bold", "c.font = Font(bold=True)"),
    "italic": ("Run / Cell", "r.italic / c.font", "r.italic", "c.font = Font(italic=True)"),
    "underline": ("Run / Cell", "r.underline / c.font", "r.underline", "c.font = Font(underline=…)"),
    "strikeout": ("Run", "r.strike", "r.font.strike", ""),
    "fontname": ("Run / Cell", "r.font / c.font", "r.font.name", "c.font = Font(name=…)"),
    "fontsize": ("Run / Cell", "r.size_pt / c.font", "r.font.size", "c.font = Font(size=…)"),
    "incfont": ("Run / Cell", "r.size_pt / c.font", "r.font.size", "c.font = Font(size=…)"),
    "decfont": ("Run / Cell", "r.size_pt / c.font", "r.font.size", "c.font = Font(size=…)"),
    "fontcolor": ("Run / Cell", "r.color / c.font", "r.font.color.rgb", "c.font = Font(color=…)"),
    "superscript": ("Run", "", "r.font.superscript", ""),
    "subscript": ("Run", "", "r.font.subscript", "c.font = Font(vertAlign=…)"),
    "clearstyle": ("Run", "r.clear()", "r.clear()", ""),
    "ruby": ("Run", "", "", ""),
    "fillparag": ("Cell", "c.fill", "", "c.fill = PatternFill(…)"),
    # **文書とセルで掛かる相手が違います。** writer は段落を枠で囲み
    # (p.boxed)、calc はセルに線を引きます。段落の枠にはまだ呼び方が
    # ありません(2026-08-25 本家のマニュアルと突き合わせて分かった)
    "borders": ("Paragraph / Cell", "c.border", "", "c.border = Border(…)"),
    "align-left": ("Paragraph / Cell", "p.align / c.alignment", "p.alignment", "c.alignment = Alignment(…)"),
    "align-center": ("Paragraph / Cell", "p.align / c.alignment", "p.alignment", "c.alignment = Alignment(…)"),
    "align-right": ("Paragraph / Cell", "p.align / c.alignment", "p.alignment", "c.alignment = Alignment(…)"),
    "align-just": ("Paragraph", "p.align = 'justify'", "p.alignment", ""),
    "align-dist": ("Paragraph", "p.align = 'distribute'", "", ""),
    "wrap": ("Cell", "c.alignment", "", "c.alignment = Alignment(wrap_text=True)"),
    "merge": ("Cell", "(col_span / v_merge) / s.merge_cells(…)", "cell.merge(…)", "ws.merge_cells('A1:B2')"),
    "parastyle": ("Paragraph", "p.style", "p.style", ""),
    "markers": ("Paragraph", "p.style = '箇条書き'", "p.style = 'List Bullet'", ""),
    "numbering": ("Paragraph", "p.style = '番号付き'", "p.style = 'List Number'", ""),
    "multilevels": ("Paragraph", "", "", ""),
    "decoffset": ("Paragraph", "p.paragraph_format", "p.paragraph_format.left_indent", ""),
    "incoffset": ("Paragraph", "p.paragraph_format", "p.paragraph_format.left_indent", ""),
    "linespace": ("Paragraph", "p.paragraph_format.line_spacing", "p.paragraph_format.line_spacing", ""),
    "replace": ("Doc", "d.replace(前, 後)", "", ""),
    "format": ("Cell", "c.number_format", "", "c.number_format"),
    "currency": ("Cell", "c.number_format", "", "c.number_format"),
    "percents": ("Cell", "c.number_format", "", "c.number_format"),
    "comma": ("Cell", "c.number_format", "", "c.number_format"),
    "cell-ins": ("Table / Sheet", "t.add_row() / s.insert_rows(行)", "t.add_row()", "ws.insert_rows(行)"),
    "cell-del": ("Sheet", "s.delete_rows(行)", "", "ws.delete_rows(行)"),
    "condformat": ("Cell", "", "", "ws.conditional_formatting.add(…)"),
    "sum": ("Cell", "s['A1'] = '=SUM(…)'", "", "ws['A1'] = '=SUM(…)'"),
    "defname": ("Book", "b.create_named_range(名前, …)", "", "wb.defined_names"),
    "sort-asc": ("Sheet", "", "", ""),
    "sort-desc": ("Sheet", "", "", ""),
    "setfilter": ("Sheet", "", "", "ws.auto_filter.ref"),
    "clear-filter": ("Sheet", "", "", "ws.auto_filter"),
    "instable": ("Doc / Sheet", "d.add_table(行, 列) / s.add_table(…)", "d.add_table(行, 列)", "ws.add_table(…)"),
    "insimage": ("Doc", "d.add_picture(径路)", "d.add_picture(径路)", "ws.add_image(…)"),
    "inschart": ("Sheet", "", "", "ws.add_chart(…)"),
    "blankpage": ("Doc", "d.add_page_break()", "d.add_page_break()", ""),
    "pagebreak": ("Doc", "d.add_page_break()", "d.add_page_break()", "ws.row_breaks"),
    "edit-header": ("Doc / Sheet", "d.header / s.oddHeader", "section.header", "ws.oddHeader"),
    "edit-footer": ("Doc", "d.footer", "section.footer", ""),
    "controls": ("Doc(記入欄)", "mcp.doc_fields()", "", ""),
    "insequation": ("Doc", "", "", ""),
    "inshyperlink": ("Cell", "c.hyperlink", "", "c.hyperlink"),
    "pivot-insert": ("Sheet", "", "", "ws.add_pivot(…)"),
    "pagemargins": ("Section", "d.sections[0]", "section.left_margin", "ws.page_margins"),
    "pageorient": ("Section", "d.sections[0]", "section.orientation", "ws.page_setup.orientation"),
    "pagesize": ("Section", "d.sections[0]", "section.page_width", "ws.page_setup.paperSize"),
    "printarea": ("Sheet", "s.print_area", "", "ws.print_area"),
    "printtitles": ("Sheet", "s.print_title_rows", "", "ws.print_title_rows"),
    "print-gridlines": ("Sheet", "s.print_gridlines", "", "ws.print_options.gridLines"),
    "insert-function": ("Cell", "s['A1'] = '=…'", "", "ws['A1'] = '=…'"),
    "calc-mode": ("Book", "b.recalc()", "", "wb.calculation"),
    "data-validation": ("Sheet", "s.add_data_validation(…)", "", "ws.add_data_validation(…)"),
    "group": ("Sheet", "s.row_groups", "", "ws.column_dimensions[…].outline_level"),
    "ungroup": ("Sheet", "s.row_groups", "", "ws.column_dimensions[…].outline_level"),
    "toc": ("Doc", "", "", ""),
    "bookmarks": ("Paragraph", "", "", ""),
    "crossref": ("Paragraph", "", "", ""),
    "footnote": ("Paragraph", "", "", ""),
    "caption": ("Paragraph", "", "", ""),
    # 記入欄。**事務の様式の中心**なので、11 個のボタンを全部載せます
    # (2026-08-25 まで、テキストフィールドと名前の2つしか載っていませんでした)。
    # 種類は docx の w:sdt に往復します。値の出し入れは名前で引くので、
    # どの種類でも呼び方は同じです
    "form-text": ("Doc(記入欄)", "mcp.doc_fill({member: 値})", "", ""),
    "form-name": ("Doc(記入欄)", "mcp.doc_fields()", "", ""),
    "form-combo": ("Doc(記入欄)", "mcp.doc_fill({member: 値})", "", ""),
    "form-dropdown": ("Doc(記入欄)", "mcp.doc_fill({member: 値})", "", ""),
    "form-checkbox": ("Doc(記入欄)", "mcp.doc_fill({member: 値})", "", ""),
    "form-radio": ("Doc(記入欄)", "mcp.doc_fill({member: 値})", "", ""),
    "form-image": ("Doc(記入欄)", "", "", ""),
    "form-email": ("Doc(記入欄)", "mcp.doc_fill({member: 値})", "", ""),
    "form-phone": ("Doc(記入欄)", "mcp.doc_fill({member: 値})", "", ""),
    "form-complex": ("Doc(記入欄)", "mcp.doc_fill({member: 値})", "", ""),
    "form-signature": ("Doc(記入欄)", "", "", ""),
    "co-addcomment": ("Comment", "p.add_comment(文) / c.comment", "p.add_comment(文)", "c.comment = Comment(…)"),
    "co-showcomment": ("Comment", "d.comments / c.comment", "d.comments", "c.comment"),
    "prot-doc": ("Sheet", "", "", "ws.protection"),
    "prot-encrypt": ("Book", "", "", "wb.security"),
    "freeze": ("Sheet", "s.freeze_panes", "", "ws.freeze_panes"),
    "show-gridlines": ("Sheet", "s.show_gridlines", "", "ws.sheet_view.showGridLines"),
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
    """
    return KAZARI.sub("", label).strip() or label


# ファイル名に使えない字。`/` は制約で置き換えるだけで、名前の一部です
FNAME_NG = re.compile(r'[/\\:*?"<>|]')
_manual_table = None


def manual_link(label: str) -> str:
    """ボタンの名前を、手引きへのリンクにして返す。

    **一覧から手引きへ飛べるようにします**(2026-08-25 発注者「一覧からの
    リンクをつける」)。この表は引くための1枚なので、行を引き当てた人が
    そのまま詳しい説明へ行けないと、そこで止まります。

    手引きがまだ無いボタンは、名前をそのまま返します(リンクにしません)。
    """
    global _manual_table
    if _manual_table is None:
        _manual_table = {}
        ahead = ROOT / "docs/ja/commands"
        for q in ahead.rglob("*.adoc"):
            if q.name != "README.ja.adoc":
                _manual_table[q.stem] = q.relative_to(ROOT / "docs").as_posix()
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


# アイコンの置き場(この文書から見た相対の径路)
ICON_DIR = "../face/icons"

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
# (段, ボタン, オブジェクト, officework, python-docx, openpyxl)
HOKA = [
    ("クイックアクセス", "保存", "Doc / Book", "d.save(径路) / b.save(径路)",
     "d.save(径路)", "wb.save(径路)"),
    ("クイックアクセス", "印刷", "Doc", "", "", ""),
    ("クイックアクセス", "元に戻す", "", "", "", ""),
    ("クイックアクセス", "やり直し", "", "", "", ""),
    ("左パネル", "見出し", "Paragraph", "p.style で拾う", "p.style", ""),
    ("左パネル", "コメント", "Comment", "d.comments", "d.comments", "c.comment"),
    ("左パネル", "検索", "Doc", "d.find(字)", "", ""),
    ("左パネル", "AI と相談する", "", "", "", ""),
    ("右パネル", "設定・ページ・スタイル", "", "", "", ""),
    # 窓の下端(ステータスバー)
    ("下端", "ページ", "Doc", "(紙に組んで数える)", "", ""),
    ("下端", "文字数", "Doc", "len(d.text)", "len(d.text)", ""),
    ("下端", "ファイルの形式", "", "", "", ""),
    ("下端", "状態の文言", "", "", "", ""),
    ("下端", "スペル", "", "", "", ""),
    ("下端", "ズーム", "", "", "", ""),
    ("下端", "選んだ範囲の合計・平均・個数", "Sheet",
     "sum(…) / len(…)(値を読んで数える)", "", ""),
    # 右クリックのメニュー(writer 17・calc 42)。**ほとんどがリボンと同じ命令**で、
    # ここにしか無い物だけを挙げます
    ("右クリック", "語を選択", "", "", "", ""),
    ("右クリック", "行を選択", "", "", "", ""),
    ("右クリック", "文字数を数える", "Doc", "len(d.text)", "len(d.text)", ""),
    # 読み飛ばした部品の一覧は officework だけの物です。
    # 本家の2つには同じ物がありません(2026-08-25 api_param_check が見つけた)
    ("右クリック", "この版で読み飛ばしたもの", "Doc", "d.unsupported", "", ""),
    ("右クリック", "形式を選択して貼り付け", "", "", "", ""),
    ("右クリック", "返信を追加", "Comment", "", "", ""),
    ("右クリック", "マクロの割り当て", "", "", "", ""),
    ("右クリック", "画像として保存(SVG)", "", "", "", ""),
    # シート見出しの右クリック(シートの管理)
    ("シート見出しの右クリック", "シートの挿入", "Book", "b.add_sheet(名前)", "", "wb.create_sheet(名前)"),
    ("シート見出しの右クリック", "シートの削除", "Book", "b.remove(名前)", "", "del wb[名前]"),
    ("シート見出しの右クリック", "シートの名前の変更", "Sheet", "s.title = 名前", "", "ws.title = 名前"),
    ("シート見出しの右クリック", "シートのコピー", "Book", "b.copy_worksheet(名前)", "", "wb.copy_worksheet(ws)"),
    ("シート見出しの右クリック", "左右へ移動", "Book", "b.move_sheet(名前, 位置)", "", "wb.move_sheet(名前, 位置)"),
    ("シート見出しの右クリック", "非表示・再表示", "Sheet", "", "", "ws.sheet_state"),
    ("シート見出しの右クリック", "タブの色", "Sheet", "", "", "ws.sheet_properties.tabColor"),
]

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
    "非表示・再表示",                    # calc/src/picks.rs
    "タブの色",
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
    "元に戻す": "画面の操作です。プログラムは保存する前の写しを持てます",
    "やり直し": "同じく画面の操作です",
    "AI と相談する": "画面の会話です。プログラムからは自分で AI を呼べます",
    "設定・ページ・スタイル": "リボンと同じボタンが並びます。上の行を見てください",
}

FILE_SRC = ROOT / "writer/src/cmds.rs"

# ファイルのページの項目 → (オブジェクト, officework, python-docx, openpyxl)。
# **リボンのファイルタブは3つしかありません**(開く・保存・印刷)。
# 実体は全面のページで、`writer/src/cmds.rs` の `file_menu()` にあります。
# リボンだけを読むと、*ファイルの仕事がほとんど表に出ません*
# (2026-08-24 発注者「どうして対応表を変更しないのだ」)
FILE_MICHI = {
    "f-new": ("Doc / Book", "Doc() / Book()", "docx.Document()", "Workbook()"),
    "f-tpl": ("Template", "", "docx.Document(雛形)", "load_workbook(雛形)"),
    "f-open": ("Doc / Book", "Doc.open(径路) / Book.open(径路)", "docx.Document(径路)", "load_workbook(径路)"),
    # **フォルダを開き直す**(2026-08-25 発注者「どうしてフォルダーを開くが
    # ないのだ」)。綴りはフォルダなので、仕事を替えるとはフォルダを替えること。
    # プログラムは径路を直に書けるので、専用の呼び方は要りません
    "f-folder": ("", "", "", ""),
    # **右パネル**(2026-08-26)。ファイル管理の口になったので載せます。
    # 作る・名前を変える・消すは Python なら pathlib で書けます
    "show-right": ("", "", "", ""),
    # **形を選んで書き出す1つの入り口**。形ごとの呼び方は save に寄せます
    "f-export": ("Doc / Book", "d.save(径路) / b.save(径路)", "d.save(径路)", "wb.save(径路)"),
    "f-url": ("Doc", "", "", ""),
    "f-recent": ("", "", "", ""),
    "f-find": ("", "", "", ""),
    "f-recover": ("", "", "", ""),
    "f-save": ("Doc / Book", "d.save(径路) / b.save(径路)", "d.save(径路)", "wb.save(径路)"),
    "f-saveas": ("Doc / Book", "d.save(別の径路)", "d.save(別の径路)", "wb.save(別の径路)"),
    "f-print": ("Doc", "", "", ""),
    "f-merge": ("Doc", "mcp.doc_merge_fields() / mcp.doc_fill(1行分)", "", ""),
    "f-html": ("Doc", "", "", ""),
    "f-protect": ("Doc / Book", "", "", "wb.security"),
    "f-distill": ("Doc", "", "", ""),
    "f-info": ("Doc / Book", "d.core_properties", "d.core_properties", "wb.properties"),
    "f-place": ("", "", "", ""),
    "f-quit": ("", "", "", ""),
    "f-opts": ("", "", "", ""),
    "f-help": ("", "", "", ""),
    "f-req": ("", "", "", ""),
    "f-back": ("", "", "", ""),
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
    return [(i, i18n_ja.japanese(keys)) for i, keys in out]


def rows():
    """(段, ボタン, 絵, オブジェクト, 印, officework, python-docx, openpyxl)。
    **並びはメニューのまま**、*分類はオブジェクト*です(2026-08-24 発注者)。"""
    tabs = ribbon_parse.tables_or_die()
    order = tab_layout(tabs)
    w = {t.name: t for t in tabs["WRITER"]}
    c = {t.name: t for t in tabs["CALC"]}
    out = []
    for tab in order:
        if tab == "ファイル":
            # **リボンの3つではなく、全面のページの一覧を出します**
            for i, label in file_menu():
                if i not in FILE_MICHI:
                    continue
                obj, ow, pd, op = FILE_MICHI[i]
                _label_lookup[i] = label
                out.append((tab, label, "", obj, state(i, ow), ow, pd, op))
            continue
        seen = set()
        for t in (w.get(tab), c.get(tab)):
            if t is None:
                continue
            for cmd in t.cmds:
                if not cmd.id or cmd.id in seen or cmd.id not in MICHI:
                    continue
                seen.add(cmd.id)
                obj, ow, pd, op = MICHI[cmd.id]
                _label_lookup[cmd.id] = cmd.label
                out.append((tab, cmd.label, cmd.icon, obj, state(cmd.id, ow), ow, pd, op))
    for tab, label, obj, ow, pd, op in HOKA:
        mark = "✅" if ow else ("❌" if label in HOKA_TSUKURANAI else "")
        _label_lookup[label] = label
        out.append((tab, label, "", obj, mark, ow, pd, op))
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
    for tab, label, *_ in r:
        keys = label
        if keys in first_seen:
            came_out[(tab, label)] = first_seen[keys]
        else:
            first_seen[keys] = tab
    return came_out


def table() -> str:
    r = rows()
    dup_of = overlap(r)
    o = []
    # **この節に説明を書きません。** 読み方はこの文書の頭にあります。
    # 利用者が読む物なので、作る側の話(生成の仕組み・作業の残り)は入れません
    o.append("")
    current = None
    for tab, label, icon, obj, st, ow, pd, op in r:
        if tab != current:
            if current is not None:
                o.append("|===\n")
            # **見出しは `==`。** `===` にすると本家が「段が飛んでいる」と
            # 警告します(この節の前に `==` が無いため。2026-08-24 に実際に出た)
            o.append(f"== {tab}")
            o.append("")
            o.append('[cols="2,2,^1,3,3,3"]')
            o.append("|===")
            o.append("|ボタン |オブジェクト |印 |officework |python-docx |openpyxl\n")
            current = tab
        f = lambda x: x if x else "—"
        inner = ow if ow else (reason(label, st) or "—")
        if (tab, label) in dup_of:
            inner = f"*{dup_of[(tab, label)]}と同じ*" + (f" — {inner}" if inner != "—" else "")
        # **絵を名前の前に出します**(2026-08-24 発注者)。画面で見ている物と
        # 同じ絵なので、名前より先に目に入ります。径路は `face/icons` から
        # この文書の場所への相対です
        # **絵の名前とファイル名は、同じとは限りません。**
        # `face/src/icons.rs` が名前とファイルを繋いでいます(例: `insertimage`
        # の実体は `insimage.svg`)。画面はそちらを通るので出ますが、
        # 文書から直に指すと届きません。ここで解いてから書きます
        name = ICON_FILE.get(icon, icon)
        icon_tag = f"image:{ICON_DIR}/{name}.svg[{label},16,16] " if name else ""
        # **ボタンの名前から手引きへ飛ばします**(2026-08-25 発注者
        # 「一覧からのリンクをつける」)。この表は引くための1枚なので、
        # 引き当てた行からそのまま詳しい説明へ行けないと途中で止まります
        o.append(f"|{icon_tag}{manual_link(label)} |{f(obj)} |{st} |{inner} |{f(pd)} |{f(op)}")
    if current is not None:
        o.append("|===\n")
    return "\n".join(o)


def cover():
    """**この表がどれだけ覆っているか**(2026-08-24)。

    「Python ですべて操作できる」と言うには、*表が全部のボタンを載せている*
    必要があります。載っていないボタンは、状態すら分かりません。
    """
    tabs = ribbon_parse.tables_or_die()
    whole = {}
    for app in ("WRITER", "CALC"):
        for tab in tabs[app]:
            for c in tab.cmds:
                if c.id:
                    whole.setdefault(c.id, (tab.name, c.label))
    # **ファイルのページも数えます**(リボンのファイルタブは3つだけで、
    # 実際の仕事は全面のページにあります)
    for i, label in file_menu():
        whole.setdefault(i, ("ファイル", label))
    # クイックアクセスと左右のパネル(リボンにもページにも無い物)
    for tab, label, *_ in HOKA:
        whole.setdefault(label, (tab, label))
    listed = [k for k in whole if k in MICHI or k in FILE_MICHI
                or any(x[1] == k for x in HOKA)]
    return len(listed), len(whole), sorted(
        (v[0], v[1], k) for k, v in whole.items()
        if k not in MICHI and k not in FILE_MICHI and not any(x[1] == k for x in HOKA)
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
