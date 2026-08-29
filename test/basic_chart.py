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
    """見本の表。1列目が区分、2列目と3列目が数"""
    b = sheet.Book()
    ws = b[0]
    for r, (na, a, c) in enumerate(
        [("東京", 3, 5), ("大阪", 1, 2), ("名古屋", 4, 1),
         ("福岡", 1, 6), ("札幌", 5, 3)], start=1
    ):
        ws.cell(r, 1).value = na
        ws.cell(r, 2).value = a
        ws.cell(r, 3).value = c
    return b, ws


# 1. 8種類とも置ける。置くたびに図形が増える
for kind in ("bar", "line", "pie", "doughnut", "area", "radar", "scatter", "bubble"):
    b, ws = moto()
    mae = len(ws.shapes)
    ws.add_chart(kind, data="B1:B5", categories="A1:A5", at="E1", title=kind)
    check(f"{kind} が置ける", len(ws.shapes) > mae, True)

# 2. 系列が2つでも置ける(散布は (x, y) の組として読む)
b, ws = moto()
ws.add_chart("bar", data="B1:C5", categories="A1:A5", at="E1")
check("系列が2つの縦棒", len(ws.shapes) > 0, True)

# 3. 知らない種類は断る。**できないことをできるように見せない**
b, ws = moto()
try:
    ws.add_chart("surface", data="B1:B5", at="E1")
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
