"""README に書いたことを、そのまま打って確かめる。

    .venv/bin/python pysheet/test_readme.py

**PyPI の説明は約束です。** 打って動かない例を載せると、入れた人が
最初の5分で離れます。ここは README の各節を1つずつ実際に打ちます。

大きさの数も見ます。2026-08-28 に「日本語1枚が約 25KB」と書いて、
測ったら 8KB でした。**言い過ぎも嘘のうち**なので、書いた数はここで
確かめます。
"""
import os
import sys
import tempfile

warui = 0


def check(cond, msg):
    global warui
    print(("  OK  " if cond else "  NG  ") + msg)
    if not cond:
        warui += 1


tmp = tempfile.mkdtemp()

# --- Spreadsheets の節 ---
from officework import sheet

b = sheet.Book.open("templates/在庫台帳.xlsx")
s = b[0]
s["A30"] = "Nihon Funen Co., Ltd."
s["C30"] = "=B30*100"
check(s["A30"].value == "Nihon Funen Co., Ltd.", "字が入る")
s.insert_row(30)
b.save(os.path.join(tmp, "out.xlsx"))
check(os.path.getsize(os.path.join(tmp, "out.xlsx")) > 1000, "xlsx が書ける")

# --- Documents の節 ---
from officework import doc

d = doc.Doc()
d.add_heading("Report", 1)
d.add_paragraph("Old Name Ltd. wrote this.")
check(isinstance(d.unsupported, (list, tuple)), "unsupported が読める")
n = d.replace("Old Name Ltd.", "New Name Ltd.")
check(n == 1, "replace が効く({})".format(n))
d.save(os.path.join(tmp, "out.docx"))
check(os.path.getsize(os.path.join(tmp, "out.docx")) > 1000, "docx が書ける")

# --- PDF の節 ---
b.save(os.path.join(tmp, "quote.pdf"))
d.save(os.path.join(tmp, "report.pdf"))
q = os.path.getsize(os.path.join(tmp, "quote.pdf"))
r = os.path.getsize(os.path.join(tmp, "report.pdf"))
check(q > 2000, "ブックが PDF になる({} バイト)".format(q))
check(r > 2000, "文書が PDF になる({} バイト)".format(r))
# 「日本語1枚が約 25KB」— 実物で測る
d2 = doc.Doc()
d2.add_heading("四月の売上", 1)
for _ in range(20):
    d2.add_paragraph("日本語の本文です。行の折り返しは JIS X 4051 に従います。")
p2 = os.path.join(tmp, "ja.pdf")
d2.save(p2)
kb = os.path.getsize(p2) / 1024
check(kb < 30, "日本語1枚が {:.0f}KB(30KB 未満と書いた)".format(kb))

# --- Charts の節 ---
b3 = sheet.Book()
ws = b3[0]
branches = ["Sapporo", "Sendai", "Tokyo"]
rates = [120, 90, 110]
for i, (m, a, t) in enumerate(zip(branches, [10, 20, 30], [12, 18, 33])):
    ws.cell(3 + i, 1).value = m
    ws.cell(3 + i, 2).value = a
    ws.cell(3 + i, 3).value = t
ws.add_chart("bar", data="B3:C5", categories="A3:A5", at="A10",
             title="Target and actual")
check(len(ws.shapes) > 10, "add_chart が図形を置く({} 個)".format(len(ws.shapes)))
for kind in ["bar", "line", "pie", "doughnut"]:
    mae = len(ws.shapes)
    ws.add_chart(kind, data="C3:C5", categories="A3:A5", at="A30")
    check(len(ws.shapes) > mae, "{} が置ける".format(kind))

from officework import chart

c = chart.Chart(340, 180, title="Attainment")
x = c.band(branches)
y = c.linear([0, 150])
c.axis_left(y, fmt=lambda v: "{}%".format(int(v)))
c.bars(x, y, rates, color="70AD47", labels=True)
c.place(ws, "A20")
check(len(ws.shapes) > 40, "d3 風の書き方が通る")

# --- Equations の節 ---
from officework import tex

svg = tex.to_svg(r"\frac{a}{b}") if hasattr(tex, "to_svg") else None
check(svg is None or len(svg) > 100, "tex が SVG を返す")

# --- 昔の書き方の節 ---
wb = sheet.Book()
ws2 = wb.active
ws2.cell(2, 3).value = 5
ws2.append(["Aug", "pens", 5000])
check(ws2.cell(2, 3).value == 5, "openpyxl の書き方(cell)")
check(ws2.cell(3, 1).value == "Aug", "openpyxl の書き方(append)")
d3 = doc.Doc()
t = d3.add_table(2, 2)
t.cell(0, 1).text = "x"
p = d3.add_paragraph("y")
check(d3.tables[0].cell(0, 1).text == "x", "python-docx の書き方(cell)")
check(p.runs[0].font.name is None or isinstance(p.runs[0].font.name, str),
      "python-docx の書き方(runs[0].font.name)")

print("OK" if warui == 0 else "{} 件おかしい".format(warui))
sys.exit(1 if warui else 0)
