#!/usr/bin/env python3
"""チャートの基本のテスト — 出せる種類が図形として置かれること。

    .venv/bin/python test/basic_chart.py

図は図形の集まりとして描くので、置いたあとシートに図形が増えます。
そこを見ます。絵が正しいかどうかは目で見て確かめます(この試験は
「置けたか」までです)。
"""
import tempfile
from pathlib import Path

from officework import sheet

tmp = Path(tempfile.mkdtemp(prefix="basic-chart-"))
BAD = []


def check(label, got, want):
    if got != want:
        BAD.append(label)
        print(f"× {label}: {want!r} のはずが {got!r}")
    else:
        print(f"  {label}: OK")


def moto():
    """見本の表。1列目が区分、2〜4列目が数。

    高安終値は3列(高値・安値・終値)、等高線は格子が要るので、
    どの種類でも足りるように**3列**用意します。
    """
    b = sheet.Book()
    ws = b[0]
    for r, (na, a, c, d) in enumerate(
        [("東京", 8, 3, 5), ("大阪", 6, 1, 2), ("名古屋", 9, 4, 6),
         ("福岡", 7, 1, 3), ("札幌", 9, 5, 7)], start=1
    ):
        ws.cell(r, 1).value = na
        ws.cell(r, 2).value = a
        ws.cell(r, 3).value = c
        ws.cell(r, 4).value = d
    return b, ws


# 1. 11種類とも置ける。置くたびに図形が増える
#    openpyxl が書ける種類と同じ数です(tools/cover_check.py が数えます)
for kind in ("bar", "line", "pie", "doughnut", "area", "radar", "scatter",
             "bubble", "stock", "surface", "projected_pie"):
    b, ws = moto()
    mae = len(ws.shapes)
    # 高安終値と等高線は列が3つ要ります
    hani = "B1:D5" if kind in ("stock", "surface") else "B1:B5"
    ws.add_chart(kind, data=hani, categories="A1:A5", at="F1", title=kind)
    check(f"{kind} が置ける", len(ws.shapes) > mae, True)

# 1b. 高安終値は列が足りなければ断る
b, ws = moto()
try:
    ws.add_chart("stock", data="B1:B5", at="F1")
    check("高安終値の列不足を断る", "受けてしまった", "断る")
except ValueError:
    check("高安終値の列不足を断る", "断る", "断る")

# 2. 系列が2つでも置ける(散布は (x, y) の組として読む)
b, ws = moto()
ws.add_chart("bar", data="B1:C5", categories="A1:A5", at="E1")
check("系列が2つの縦棒", len(ws.shapes) > 0, True)

# 3. 知らない種類は断る。**できないことをできるように見せない**
b, ws = moto()
try:
    ws.add_chart("smiley", data="B1:B5", at="E1")
    check("知らない種類を断る", "受けてしまった", "断る")
except ValueError:
    check("知らない種類を断る", "断る", "断る")

# 4. 置いた図は xlsx に残る(図形として書かれる)
b, ws = moto()
ws.add_chart("area", data="B1:B5", categories="A1:A5", at="E1")
p = tmp / "chart.xlsx"
b.save(str(p))
check("xlsx の頭", p.read_bytes()[:2], b"PK")
b2 = sheet.Book.open(str(p))
check("開き直しても図形がある", len(b2[0].shapes) > 0, True)

# 5. 紙にも出る
q = tmp / "chart.pdf"
b.save(str(q))
check("PDF の頭", q.read_bytes()[:5], b"%PDF-")

if BAD:
    raise SystemExit(f"合わない物 {len(BAD)} 件: " + "、".join(BAD))
print("基本のテスト: 全部合った")
