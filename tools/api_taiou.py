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
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).parent))
import ribbon_parse  # noqa: E402

# writer: id → (officework.doc, python-docx)
DOC = {
    "open": ("doc.Doc.open(径路)", "docx.Document(径路)"),
    "save": ("d.save(径路)", "d.save(径路)"),
    "parastyle": ("p.style", "p.style"),
    "markers": ("p.style = '箇条書き'", "p.style = 'List Bullet'"),
    "numbering": ("p.style = '番号付き'", "p.style = 'List Number'"),
    "bold": ("r.bold", "r.bold"),
    "italic": ("r.italic", "r.italic"),
    "underline": ("r.underline", "r.underline"),
    "strikeout": ("r.strike", "r.font.strike"),
    "fontname": ("r.font", "r.font.name"),
    "fontsize": ("r.size_pt", "r.font.size"),
    "incfont": ("r.size_pt", "r.font.size"),
    "decfont": ("r.size_pt", "r.font.size"),
    "fontcolor": ("r.color", "r.font.color.rgb"),
    "clearstyle": ("r.clear()", "r.clear()"),
    "align-left": ("p.align = 'left'", "p.alignment"),
    "align-center": ("p.align = 'center'", "p.alignment"),
    "align-right": ("p.align = 'right'", "p.alignment"),
    "align-just": ("p.align = 'justify'", "p.alignment"),
    "align-dist": ("p.align = 'distribute'", ""),
    "linespace": ("p.paragraph_format.line_spacing", "p.paragraph_format.line_spacing"),
    "decoffset": ("p.paragraph_format", "p.paragraph_format.left_indent"),
    "incoffset": ("p.paragraph_format", "p.paragraph_format.left_indent"),
    "replace": ("d.replace(前, 後)", ""),
    "instable": ("d.add_table(行, 列)", "d.add_table(行, 列)"),
    "insimage": ("d.add_picture(径路)", "d.add_picture(径路)"),
    "blankpage": ("d.add_page_break()", "d.add_page_break()"),
    "pagebreak": ("d.add_page_break()", "d.add_page_break()"),
    "edit-header": ("d.header", "section.header"),
    "edit-footer": ("d.footer", "section.footer"),
    "controls": ("d.fields()", ""),
    "form-text": ("d.fill(名前, 値)", ""),
    "form-name": ("d.fields()", ""),
    "pagemargins": ("d.sections[0]", "section.left_margin"),
    "pageorient": ("d.sections[0]", "section.orientation"),
    "pagesize": ("d.sections[0]", "section.page_width"),
    "co-addcomment": ("p.add_comment(文, author=)", "p.add_comment(文)"),
    "co-showcomment": ("d.comments", "d.comments"),
    "ruby": ("", ""),
    "superscript": ("", "r.font.superscript"),
    "subscript": ("", "r.font.subscript"),
    "footnote": ("", ""),
    "bookmarks": ("", ""),
    "crossref": ("", ""),
    "toc": ("", ""),
    "caption": ("", ""),
    "insequation": ("", ""),
    "pdf": ("", ""),
}

# calc: id → (officework.sheet, openpyxl)
SHEET = {
    "open": ("sheet.Book.open(径路)", "load_workbook(径路)"),
    "save": ("b.save(径路)", "wb.save(径路)"),
    "copy": ("s['A1'] = 値", "ws['A1'] = 値"),
    "cut": ("s['A1'] = 値", "ws['A1'] = 値"),
    "paste": ("s['A1'] = 値", "ws['A1'] = 値"),
    "clear": ("s['A1'] = None", "ws['A1'] = None"),
    "sum": ("s['A1'] = '=SUM(…)'", "ws['A1'] = '=SUM(…)'"),
    "insert-function": ("s['A1'] = '=…'", "ws['A1'] = '=…'"),
    "cell-ins": ("s.insert_rows(行)", "ws.insert_rows(行)"),
    "cell-del": ("s.delete_rows(行)", "ws.delete_rows(行)"),
    "merge": ("s.merge_cells('A1:B2')", "ws.merge_cells('A1:B2')"),
    "fontname": ("c.font", "c.font = Font(name=…)"),
    "fontsize": ("c.font", "c.font = Font(size=…)"),
    "bold": ("c.font", "c.font = Font(bold=True)"),
    "italic": ("c.font", "c.font = Font(italic=True)"),
    "underline": ("c.font", "c.font = Font(underline=…)"),
    "fontcolor": ("c.font", "c.font = Font(color=…)"),
    "fillparag": ("c.fill", "c.fill = PatternFill(…)"),
    "borders": ("c.border", "c.border = Border(…)"),
    "align-left": ("c.alignment", "c.alignment = Alignment(…)"),
    "align-center": ("c.alignment", "c.alignment = Alignment(…)"),
    "align-right": ("c.alignment", "c.alignment = Alignment(…)"),
    "wrap": ("c.alignment", "c.alignment = Alignment(wrap_text=True)"),
    "format": ("c.number_format", "c.number_format"),
    "currency": ("c.number_format", "c.number_format"),
    "percents": ("c.number_format", "c.number_format"),
    "comma": ("c.number_format", "c.number_format"),
    "defname": ("b.create_named_range(名前, …)", "wb.defined_names"),
    "condformat": ("", "ws.conditional_formatting.add(…)"),
    "data-validation": ("s.add_data_validation(…)", "ws.add_data_validation(…)"),
    "setfilter": ("", "ws.auto_filter.ref"),
    "clear-filter": ("", "ws.auto_filter"),
    "group": ("s.row_groups", "ws.column_dimensions[…].outline_level"),
    "ungroup": ("s.row_groups", "ws.column_dimensions[…].outline_level"),
    "freeze": ("s.freeze_panes", "ws.freeze_panes"),
    "instable": ("s.add_table(…)", "ws.add_table(…)"),
    "insimage": ("", "ws.add_image(…)"),
    "inschart": ("", "ws.add_chart(…)"),
    "pivot-insert": ("", "ws.add_pivot(…)"),
    "co-addcomment": ("c.comment", "c.comment = Comment(…)"),
    "inshyperlink": ("c.hyperlink", "c.hyperlink"),
    "printarea": ("s.print_area", "ws.print_area"),
    "printtitles": ("s.print_title_rows", "ws.print_title_rows"),
    "print-gridlines": ("s.print_gridlines", "ws.print_options.gridLines"),
    "pagemargins": ("", "ws.page_margins"),
    "pageorient": ("", "ws.page_setup.orientation"),
    "pagesize": ("", "ws.page_setup.paperSize"),
    "edit-header": ("s.oddHeader", "ws.oddHeader"),
    "show-gridlines": ("s.show_gridlines", "ws.sheet_view.showGridLines"),
    "prot-doc": ("", "ws.protection"),
    "prot-encrypt": ("", "wb.security"),
    "calc-mode": ("b.recalc()", "wb.calculation"),
    "sort-asc": ("", ""),
    "sort-desc": ("", ""),
    "python": ("", ""),
    "pdf": ("", ""),
}

MARK_S = "// api:taiou:start"
MARK_E = "// api:taiou:end"
SAKI = ROOT / "docs/api-taiou.ja.adoc"


def rows():
    """(アプリ, タブ, ボタン, officework, 本家)"""
    tabs = ribbon_parse.tables_or_die()
    out = []
    for app, tbl in (("writer", DOC), ("calc", SHEET)):
        key = "WRITER" if app == "writer" else "CALC"
        seen = set()
        for tab in tabs[key]:
            for c in tab.cmds:
                if not c.id or c.id not in tbl or c.id in seen:
                    continue
                seen.add(c.id)
                ours, theirs = tbl[c.id]
                out.append((app, tab.name, c.label, ours, theirs))
    return out


def 表() -> str:
    r = rows()
    o = []
    o.append("1行が1つの操作です。画面のボタン・`officework`・本家のライブラリが横に並びます。")
    o.append("空いている所(—)は、その道が無いという意味です。\n")
    o.append("この節は `tools/api_taiou.py` が起こします。手で直さないでください。\n")
    for app, honke, midashi in (("writer", "python-docx", "文書(writer)"),
                                ("calc", "openpyxl", "表(calc)")):
        o.append(f"=== {midashi}")
        o.append("")
        o.append('[cols="1,2,3,3"]')
        o.append("|===")
        o.append(f"|タブ |ボタン |officework |{honke}\n")
        for a, tab, label, ours, theirs in r:
            if a != app:
                continue
            o.append(f"|{tab} |{label} |{ours or '—'} |{theirs or '—'}")
        o.append("|===\n")
    return "\n".join(o)


def main() -> int:
    src = SAKI.read_text(encoding="utf-8")
    m = re.search(rf"({re.escape(MARK_S)}[^\n]*\n)(.*?)(\n?{re.escape(MARK_E)})", src, re.S)
    if not m:
        print(f"::error::{SAKI.name} に {MARK_S} の印がありません", file=sys.stderr)
        return 1
    beki = 表()
    if "--write" in sys.argv:
        SAKI.write_text(src[: m.start(2)] + beki + src[m.end(2):], encoding="utf-8")
        print(f"{SAKI.name} を書き直しました({len(rows())} 行)")
        return 0
    if m.group(2).strip() != beki.strip():
        print(f"::error::{SAKI.name} の対応表が実物とずれています"
              "(python3 tools/api_taiou.py --write で直します)", file=sys.stderr)
        return 1
    print(f"対応表は実物と揃っています({len(rows())} 行)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
