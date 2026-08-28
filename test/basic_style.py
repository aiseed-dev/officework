"""スタイル定義の性質が往復するか。

    .venv/bin/python test/basic_style.py

台帳で「名乗りだけの最小定義をどこまで広げるか — 決めてから」と
保留していた項目です(2026-08-28 に発注者から着手の指示)。

見るのは、本家(python-docx)が持つ物と同じ呼び名で読み書きできて、
保存したものを**本家の目でも同じに読める**かどうかです。
"""
import os
import sys
import tempfile

import docx
from docx.enum.text import WD_ALIGN_PARAGRAPH

from officework import _doc as od

warui = 0


def check(cond, msg):
    global warui
    print(("  OK  " if cond else "× ") + msg)
    if not cond:
        warui += 1


tmp = tempfile.mkdtemp(prefix="style-")
f = os.path.join(tmp, "s.docx")

d = od.Doc()
st = d.styles.add_style("注記", "paragraph")
st.base_style = d.styles["Title"]
st.hidden = False
st.locked = True
st.quick_style = True
st.priority = 42
st.paragraph_format.alignment = WD_ALIGN_PARAGRAPH.RIGHT
st.paragraph_format.space_before = od.Pt(12)
st.paragraph_format.line_spacing = 1.5
st.paragraph_format.first_line_indent = od.Pt(10.5)
d.add_paragraph("本文", style="注記")
d.save(f)

# ① うちで読み返す
b = od.Doc.open(f)
s2 = b.styles["注記"]
pf = s2.paragraph_format
check(s2.base_style is not None and s2.base_style.name == "Title", "元になるスタイル")
check(s2.quick_style is True, "リボンの一覧に出す")
check(s2.locked is True, "書き替えを禁じる")
check(s2.priority == 42, "並べる順: {}".format(s2.priority))
check(pf.alignment == "right", "揃え: {}".format(pf.alignment))
check(abs(pf.space_before.pt - 12) < 0.1, "前の空き: {}".format(pf.space_before))
check(abs(pf.line_spacing - 1.5) < 0.01, "行間: {}".format(pf.line_spacing))
check(abs(pf.first_line_indent.pt - 10.5) < 0.1,
      "1行目の字下げ: {}".format(pf.first_line_indent))

# ② 本家の目でも同じに読める
w = docx.Document(f)
ws = w.styles["注記"]
check(ws.base_style is not None and ws.base_style.name == "Title", "本家の目(元)")
check(ws.quick_style is True, "本家の目(リボン)")
check(ws.priority == 42, "本家の目(順)")
check(ws.paragraph_format.alignment == WD_ALIGN_PARAGRAPH.RIGHT, "本家の目(揃え)")

# ③ 原本のスタイルの性質も書ける(据え置きの中で、触った定義だけ差し替え)
b2 = od.Doc.open(f)
b2.styles["Title"].priority = 7
g = os.path.join(tmp, "t.docx")
b2.save(g)
check(od.Doc.open(g).styles["Title"].priority == 7, "原本のスタイルも書ける")
check(od.Doc.open(g).styles["注記"].priority == 42, "触っていない定義は残る")

# ④ 消せるのは自作の物だけ(原本の物は正直に断る)
b3 = od.Doc.open(f)
try:
    b3.styles["Title"].delete()
    check(False, "原本のスタイルを黙って消した")
except Exception:
    check(True, "原本のスタイルは消せないと言う")

print("OK" if warui == 0 else "{} 件おかしい".format(warui))
sys.exit(1 if warui else 0)
