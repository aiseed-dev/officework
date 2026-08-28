#!/usr/bin/env python3
"""PDF の基本のテスト — save() が拡張子で PDF を書くこと。

    .venv/bin/python test/basic_pdf.py

見るのは基本だけです: 表と文書のそれぞれで save("〜.pdf") が本物の
PDF(%PDF で始まる)を書き、同じ物を .xlsx / .docx にも書き分けること。
"""
import tempfile
from pathlib import Path

from officework import sheet, doc

tmp = Path(tempfile.mkdtemp(prefix="basic-pdf-"))
BAD = []


def check(label, got, want):
    if got != want:
        BAD.append(label)
        print(f"× {label}: {want!r} のはずが {got!r}")
    else:
        print(f"  {label}: OK")


# 1. 表 → PDF(式は計算されて紙になる)
b = sheet.Book()
ws = b[0]
ws.cell(1, 1).value = "品名"
ws.cell(1, 2).value = "金額"
ws.cell(2, 1).value = "ペン"
ws.cell(2, 2).value = "=5*100"
b.recalc()
p = tmp / "quote.pdf"
b.save(str(p))
head = p.read_bytes()[:5]
check("表の PDF の頭", head, b"%PDF-")
check("表の PDF に中身がある", p.stat().st_size > 500, True)

# 2. 同じブックを .xlsx にも書き分けられる
px = tmp / "quote.xlsx"
b.save(str(px))
check("同じブックの xlsx の頭", px.read_bytes()[:2], b"PK")

# 3. 文書 → PDF
d = doc.Doc()
d.add_heading("試しの文書", level=1)
d.add_paragraph("PDF の基本の確かめです。日本語も含みます。")
q = tmp / "report.pdf"
d.save(str(q))
check("文書の PDF の頭", q.read_bytes()[:5], b"%PDF-")
check("文書の PDF に中身がある", q.stat().st_size > 500, True)

# 4. 同じ文書を .docx にも書き分けられる
qd = tmp / "report.docx"
d.save(str(qd))
check("同じ文書の docx の頭", qd.read_bytes()[:2], b"PK")

# 5. 書体の名指し — 文書ぜんたいの書体が PDF に埋まる
#    (この機械に BIZ UD 書体が入っている前提。細かくは pysheet/test_font.py)
d2 = doc.Doc()
d2.font = "BIZ UD明朝"
d2.add_paragraph("書体の名指しの確かめです。")
q2 = tmp / "mincho.pdf"
d2.save(str(q2))
check("名指しした書体が埋まる", b"BIZUDMincho" in q2.read_bytes(), True)

if BAD:
    raise SystemExit(f"合わない物 {len(BAD)} 件: " + "、".join(BAD))
print("基本のテスト: 全部合った")
