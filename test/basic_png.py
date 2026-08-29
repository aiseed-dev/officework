#!/usr/bin/env python3
"""PNG の基本のテスト — save() が拡張子で絵を書くこと。

    .venv/bin/python test/basic_png.py

見るのは基本だけです: 表と文書のそれぞれで save("〜.png") が本物の
PNG を書き、dpi で大きさが変わり、頁が増えたら名前に番号が付くこと。
絵が正しいかどうかは paper の回帰検査(paper/tests/kaiki.rs)が見ます。
"""
import struct
import tempfile
from pathlib import Path

from officework import sheet, doc

tmp = Path(tempfile.mkdtemp(prefix="basic-png-"))
BAD = []

# PNG の頭の8バイト
SHIRUSHI = b"\x89PNG\r\n\x1a\n"


def check(label, got, want):
    if got != want:
        BAD.append(label)
        print(f"× {label}: {want!r} のはずが {got!r}")
    else:
        print(f"  {label}: OK")


def ookisa(p):
    """PNG の縦横を頭から読む(IHDR の最初の8バイト)"""
    b = p.read_bytes()
    return struct.unpack(">II", b[16:24])


# 1. 表 → PNG(式は計算されて絵になる)
b = sheet.Book()
ws = b[0]
ws.cell(1, 1).value = "品名"
ws.cell(1, 2).value = "金額"
ws.cell(2, 1).value = "ペン"
ws.cell(2, 2).value = "=5*100"
b.recalc()
p = tmp / "quote.png"
b.save(str(p))
check("表の PNG の頭", p.read_bytes()[:8], SHIRUSHI)
check("表の PNG に中身がある", p.stat().st_size > 1000, True)
# 既定は 150 dpi。A4 なら 1240×1754 画素
check("既定の大きさ", ookisa(p), (1240, 1754))

# 2. 同じブックを .xlsx にも書き分けられる
px = tmp / "quote.xlsx"
b.save(str(px))
check("同じブックの xlsx の頭", px.read_bytes()[:2], b"PK")

# 3. dpi で細かさが変わる
p300 = tmp / "quote300.png"
b.save(str(p300), dpi=300)
check("300 dpi の大きさ", ookisa(p300), (2480, 3508))

# 4. 細かさが桁違いなら断る(機械を止めずに断る)
for warui in (0, -100, 100000):
    try:
        b.save(str(tmp / "warui.png"), dpi=warui)
        check(f"dpi={warui} を断る", "受けてしまった", "断る")
    except OSError:
        check(f"dpi={warui} を断る", "断る", "断る")

# 5. 文書 → PNG
d = doc.Doc()
d.add_heading("試しの文書", level=1)
d.add_paragraph("PNG の基本の確かめです。日本語も含みます。")
q = tmp / "report.png"
d.save(str(q))
check("文書の PNG の頭", q.read_bytes()[:8], SHIRUSHI)
check("文書の PNG に中身がある", q.stat().st_size > 1000, True)

# 5. 同じ文書を .docx にも書き分けられる
qd = tmp / "report.docx"
d.save(str(qd))
check("同じ文書の docx の頭", qd.read_bytes()[:2], b"PK")

# 6. 頁が増えたら名前に番号が付く。**1枚目は渡した名前のまま**
nagai = doc.Doc()
for i in range(1, 401):
    nagai.add_paragraph(f"{i} 行目の本文です。ここは頁を溢れさせるための字です。")
r = tmp / "nagai.png"
nagai.save(str(r))
check("1枚目は渡した名前のまま", r.read_bytes()[:8], SHIRUSHI)
check("2枚目に番号が付く", (tmp / "nagai-2.png").read_bytes()[:8], SHIRUSHI)

if BAD:
    raise SystemExit(f"合わない物 {len(BAD)} 件: " + "、".join(BAD))
print("基本のテスト: 全部合った")
