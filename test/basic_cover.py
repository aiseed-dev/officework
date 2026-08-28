"""**出発点の3つができることが、全部できるか。**

    .venv/bin/python test/basic_cover.py

発注者(2026-08-29)「出発点を Euro-Office と python-docx と openpyxl に
している。これらができることを、すべてできるようにするのが今回の目的」。

`tools/cover_check.py` が名前の穴を数えます。ここは**足した口が実際に
動くか**を見ます。名前だけ生やして中身が無い、を防ぐためです。
"""
import sys

from officework import _doc as od
from officework import sheet

warui = 0


def check(cond, msg):
    global warui
    print(("  OK  " if cond else "× ") + msg)
    if not cond:
        warui += 1


b = sheet.Book()
ws = b[0]
ws["A1"] = "品名"

# --- openpyxl の Workbook ---
check(b.data_only is False, "data_only(こちらは式も値も常に持つ)")
check(b.read_only is False, "read_only")
check(b.chartsheets == [], "chartsheets(まだ持たない)")
check(b.get_sheet_names() == list(b.sheetnames), "get_sheet_names")
check(b.get_sheet_by_name(b.sheetnames[0]).name == b.sheetnames[0], "get_sheet_by_name")
try:
    b.create_chartsheet()
    check(False, "グラフだけのシートを黙って作った")
except NotImplementedError:
    check(True, "create_chartsheet は正直に断る")

# --- openpyxl の Worksheet ---
check(ws.active_cell == "A1", "active_cell")
check(ws.selected_cell == "A1", "selected_cell")
ws.sheet_view.showGridLines = False
check(ws.show_gridlines is False, "sheet_view.showGridLines")
ws.sheet_view.zoomScale = 120
check(ws.zoom_scale == 120, "sheet_view.zoomScale")
ws.set_printer_settings(paper_size=9, orientation="landscape")
check((ws.paper_size, ws.orientation) == (9, "landscape"), "set_printer_settings")

# --- openpyxl の Cell ---
c = ws["A1"]
check(c.internal_value == "品名", "internal_value")
check(c.quotePrefix is False, "quotePrefix")
check(c.pivotButton is False, "pivotButton")
check(c.check_error() is False, "check_error(普通の字)")
ws["A2"] = "#N/A"
check(ws["A2"].check_error() is True, "check_error(エラーの字)")
check(c.check_string("あ" * 40000) == "あ" * 32767, "check_string(32,767 字で切る)")
check(c.has_style is False, "has_style")

# --- python-docx ---
d = od.Doc()
p = d.add_paragraph("本文")
r = p.runs[0]
check(p.contains_page_break is False, "Paragraph.contains_page_break(既定)")
p.paragraph_format.page_break_before = True
check(p.contains_page_break is True, "改ページを入れた後")
check(len(p.rendered_page_breaks) == 1, "rendered_page_breaks")
check(r.contains_page_break is False, "Run.contains_page_break")
d.add_comment("見てください", "甲")
check(len(d[len(d) - 1].comments) == 1, "Document.add_comment")
check(d.settings is not None, "settings")
t = d.add_table(1, 1)
check(t.table is t, "Table.table")
check(t.table_direction is None, "table_direction")
try:
    t.table_direction = "rtl"
    check(False, "右から左の表を黙って受けた")
except NotImplementedError:
    check(True, "table_direction は正直に断る")

print("OK" if warui == 0 else "{} 件おかしい".format(warui))
sys.exit(1 if warui else 0)
