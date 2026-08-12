"""openpyxl・xlwings・python-docx の公開 API の棚卸し。

docs/pysheet-gokan.ja.md(互換台帳)の生データを採る道具。
3つが .venv に入っている前提(pip install openpyxl xlwings python-docx)。

    .venv/bin/python tools/gokan_inventory.py inventory.json
"""
import inspect, json, sys

def survey(classes):
    out = {}
    for cls in classes:
        members = [n for n, _ in inspect.getmembers(cls) if not n.startswith("_")]
        out[cls.__module__ + "." + cls.__name__] = {"count": len(members), "members": sorted(members)}
    return out

result = {}

# openpyxl — 実務でよく使う中核クラス
from openpyxl.workbook import Workbook
from openpyxl.worksheet.worksheet import Worksheet
from openpyxl.cell.cell import Cell
result["openpyxl"] = survey([Workbook, Worksheet, Cell])

# xlwings — 主要4クラス
import xlwings
result["xlwings"] = survey([xlwings.App, xlwings.Book, xlwings.Sheet, xlwings.Range])

# python-docx — 主要4クラス
from docx.document import Document
from docx.text.paragraph import Paragraph
from docx.text.run import Run
from docx.table import Table
result["python-docx"] = survey([Document, Paragraph, Run, Table])

for lib, classes in result.items():
    print(f"== {lib}")
    for name, info in classes.items():
        print(f"  {name}: {info['count']}")

if len(sys.argv) > 1:
    with open(sys.argv[1], "w") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)
