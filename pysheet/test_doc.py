# officework.doc(pysheet)の検査。Rust 側の tests/python_smoke.rs から呼ばれる。
# 手で回すなら:
#   cargo build -p pysheet
#   cp target/debug/lib_sheet.so pysheet/officework/_sheet.so
#   PYTHONPATH=pysheet python3 pysheet/test_doc.py
import os
import re
import sys
import tempfile
import zipfile

from officework import doc


def check(cond, msg):
    if not cond:
        print(f"NG: {msg}", file=sys.stderr)
        sys.exit(1)


# 実物の docx。試験の材料はリポジトリの sample/(writer が読み書きしている物)
HERE = os.path.dirname(os.path.abspath(__file__))
SAMPLE = os.environ.get("DOCX", os.path.join(HERE, "..", "sample", "報告書.docx"))

# --- 空の文書を作って往復する ------------------------------------------------
d = doc.Doc()
check(len(d) == 0, "作りたての文書に段落がある")
p = d.add_paragraph("ザボガードF F-02")
check(p.text == "ザボガードF F-02", "足した段落が読めない")
check(len(d) == 1, "足した段落が数に入らない")

with tempfile.TemporaryDirectory() as t:
    out = os.path.join(t, "round.docx")
    d.save(out)
    d2 = doc.Doc.open(out)
    check(d2[0].text == "ザボガードF F-02", "日本語が往復しない")

# --- 実物を読む --------------------------------------------------------------
if not os.path.exists(SAMPLE):
    print(f"実物の docx({SAMPLE})が無いので飛ばした", file=sys.stderr)
    print("OK")
    sys.exit(0)

d = doc.Doc.open(SAMPLE)
check(d.unsupported == [], f"読めなかった部品がある: {d.unsupported}")
check(len(d) > 0, "実物の段落が読めない")
check(len(d.paragraphs) == len(d), "paragraphs と len が食い違う")
check(d[0].text != "", "1段落目が空")
check(d[-1] is not None and d[-1].text == d.paragraphs[-1].text, "負の添字が効かない")
check(d.text.count("\n") == len(d) - 1, "本文が段落の数だけ改行で繋がっていない")

# --- 表(表・行・セルの3階建て)----------------------------------------------
check(len(d.tables) > 0, "実物の表が読めない")
t0 = d.tables[0]
rows, cols = t0.shape
check(rows > 0 and cols > 0, f"表の大きさが取れない: {t0.shape}")
check(len(t0) == rows, "len(表) が行数でない")
check(len(t0[0]) > 0, "行に列が無い")
head = t0[0][0].text
check(head != "", "セルが空")
check(t0.values()[0][0] == head, "values() と [行][列] が食い違う")

# --- text の代入: 段落の性質は据え置き、run は先頭の書式を継ぐ ----------------
# 見出しに代入しても見出しのまま(帳票の様式が壊れないのが存在理由)
head_i = next((i for i, p in enumerate(d.paragraphs) if p.style.startswith("heading")), None)
check(head_i is not None, "見出しが1つも読めていない")
hp = d[head_i]
style_before, align_before = hp.style, hp.align
was_bold = hp.runs[0].bold if hp.runs else False
hp.text = "差し替えた見出し"
check(hp.text == "差し替えた見出し", "代入した字が読み戻せない")
check(hp.style == style_before, f"代入で段落の役目が変わった: {hp.style}")
check(hp.align == align_before, "代入で行の寄せが変わった")
check(len(hp.runs) == 1, "代入で run が1本にまとまっていない")
check(hp.runs[0].bold == was_bold, "代入で先頭 run の書式を継いでいない")
try:
    hp.text = "改行を\n入れる"
    check(False, "段落に改行を黙って入れた")
except ValueError:
    pass

# --- replace: run の切れ目を残す(差し込みの本筋)-----------------------------
d = doc.Doc.open(SAMPLE)
needle = d[0].text[:3]
check(len(d.find(needle)) > 0, f"find が実物で何も拾わない: {needle!r}")
runs_before = len(d[0].runs)
n = d.replace(needle, "○" * len(needle))
check(n > 0, "replace が実物で1つも置き換えない")
check(len(d[0].runs) == runs_before, "replace で run の数が変わった(書式の分かれ目が消えた)")
check(d.replace("この字は入っていないはず", "x") == 0, "無い字を置き換えたことになっている")

# --- セルへの代入 ------------------------------------------------------------
d = doc.Doc.open(SAMPLE)
c = d.tables[0][1][0]
c.text = "2026-08-10"
check(c.text == "2026-08-10", "セルへの代入が読み戻せない")
check(d.tables[0][1][0].paragraphs[0].in_table, "セルの中の段落が in_table でない")

# --- 保存: 原本を正として、変えた所だけ書き戻す -------------------------------
# **これが売り文句の土台。** python-docx は理解できない部品を書き直してしまう
with zipfile.ZipFile(SAMPLE) as z:
    parts_before = set(z.namelist())

d = doc.Doc.open(SAMPLE)
first_before = d[0].text
d.replace("業務", "作業")
with tempfile.TemporaryDirectory() as t:
    out = os.path.join(t, "差込.docx")
    d.save(out)
    with zipfile.ZipFile(out) as z:
        parts_after = set(z.namelist())
    lost = parts_before - parts_after
    check(not lost, f"保存で原本の部品が消えた: {sorted(lost)}")

    d3 = doc.Doc.open(out)
    check(d3.unsupported == [], f"保存した物が読み直せない: {d3.unsupported}")
    check(len(d3) == len(d), "保存で段落の数が変わった")
    check(len(d3.tables) == len(d.tables), "保存で表が消えた")
    check(d3[0].text == first_before.replace("業務", "作業"), "差し込んだ字が保存されない")
    check(d3.tables[0].values() == d.tables[0].values(), "保存で表の中身が変わった")

# ── to_pdf は save(".pdf") と同じ道 ────────────────────────────────
# 名前を分けたのは、PDF だけの指定を足す置き場を作るためです
# (2026-08-30 発注者)。中身が分かれてしまわないよう、同じ PDF が
# 出ることをここで見ます。
with tempfile.TemporaryDirectory() as t:
    d = doc.Doc(lang="ja")
    d.add_paragraph("見本の文書です")
    d.add_paragraph("2段落目")
    a, b = os.path.join(t, "a.pdf"), os.path.join(t, "b.pdf")
    d.save(a)
    modori = d.to_pdf(b)
    check(open(a, "rb").read() == open(b, "rb").read(),
          "to_pdf が save('.pdf') と違う PDF を出した")
    check(modori == b, f"to_pdf の返りが保存先でない: {modori}")

    # 名前を省くと、開いたファイルの名前の .pdf
    docx = os.path.join(t, "報告.docx")
    d.save(docx)
    p = doc.Doc.open(docx).to_pdf()
    check(p == os.path.join(t, "報告.pdf"), f"省いたときの名前が違う: {p}")
    check(os.path.exists(p), "省いたときに書かれていない")

# ── 節の用紙は docx にも全部書く ──────────────────────────────────
# 前は途中の節の sectPr が空で、余白も書き替えた辺しか入らず、
# 開いたソフトの既定(英語圏の Word なら Letter)になっていました
# (2026-08-30)。
with tempfile.TemporaryDirectory() as t:
    d = doc.Doc(lang="ja")
    d.add_paragraph("1節")
    s = d.add_section()
    s.page_width, s.page_height = 254 * 36000, 180 * 36000
    s.left_margin = 30 * 36000
    d.add_paragraph("2節")
    out = os.path.join(t, "節.docx")
    d.save(out)
    with zipfile.ZipFile(out) as z:
        x = z.read("word/document.xml").decode("utf-8")
    sects = re.findall(r"<w:sectPr.*?</w:sectPr>|<w:sectPr[^>]*/>", x, re.S)
    check(len(sects) == 2, f"節が2つでない: {len(sects)}")
    for i, sx in enumerate(sects):
        check("w:pgSz" in sx, f"節{i + 1}に紙の大きさが無い")
        for hen in ("w:top", "w:right", "w:bottom", "w:left"):
            check(hen in sx, f"節{i + 1}に {hen} の余白が無い")
    check('w:orient="landscape"' in sects[1], "横長の節に向きの印が無い")
    check('w:w="11906"' in sects[0], f"1節目が A4 でない: {sects[0]}")

# ── 段落のスタイルは日本語の別名と heading4〜9・tof も受ける ─────────────
# 手引きが `p.style = '箇条書き'` と書いているのに断っていました
d = doc.Doc()
p = d.add_paragraph("一")
p.style = "箇条書き"
check(p.style == "List Bullet", f"箇条書きが List Bullet にならない: {p.style}")
p = d.add_paragraph("二")
p.style = "番号付き"
check(p.style == "List Number", f"番号付きが List Number にならない: {p.style}")
for name in ("heading4", "heading9", "tof", "toc2", "見出し5"):
    p = d.add_paragraph(name)
    p.style = name
    yomi = {"見出し5": "heading5"}.get(name, name)
    check(p.style == yomi, f"{name} を受けない: {p.style}")

# ── add_section("continuous") の始め方は新しい節に付く ────────────────
# 前は1つ前の節に付いて見えました(python-docx は新しい節に付けます)
d = doc.Doc()
d.add_paragraph("1節")
s2 = d.add_section("continuous")
d.add_paragraph("2節")
check(d.sections[0].start_type == "new_page", f"1節目が {d.sections[0].start_type}")
check(s2.start_type == "continuous", f"足した節が {s2.start_type}")
check(d.sections[-1].start_type == "continuous", "末尾の節に始め方が付かない")
s3 = d.add_section()
check(s3.start_type == "new_page" and d.sections[1].start_type == "continuous",
      "3節目を足すと2節目の始め方が変わる")
with tempfile.TemporaryDirectory() as t:
    out = os.path.join(t, "節.docx")
    d.save(out)
    with zipfile.ZipFile(out) as z:
        x = z.read("word/document.xml").decode("utf-8")
    sects = re.findall(r"<w:sectPr.*?</w:sectPr>|<w:sectPr[^>]*/>", x, re.S)
    check(len(sects) == 3, f"節が3つでない: {len(sects)}")
    check('w:val="continuous"' in sects[0], "continuous の印が docx に無い")
    check('w:val="continuous"' not in sects[2], "末尾の節に continuous が残る")
    d2 = doc.Doc(out)
    check([s.start_type for s in d2.sections] == ["new_page", "continuous", "new_page"],
          f"読み直した始め方が違う: {[s.start_type for s in d2.sections]}")

# ── 差し込み(render)と記入欄(fill)、ページ数 ────────────────────────
d = doc.Doc()
d.add_paragraph("宛先 {{宛名}} 様 合計 {{合計}} 円")
r = d.render({"宛名": "サンプル商事株式会社", "合計": 1440000})
check(d.paragraphs[0].text == "宛先 サンプル商事株式会社 様 合計 1440000 円",
      f"render で差し込めない: {d.paragraphs[0].text}")
try:
    d.fill("無い欄", "x")
    check(False, "無い記入欄に fill が黙って成功する")
except KeyError:
    pass
check(d.page_count() == 1, f"1枚の文書のページ数が {d.page_count()}")
for i in range(80):
    d.add_paragraph("行 %d" % i)
check(d.page_count() >= 2, f"80 行の文書のページ数が {d.page_count()}")

print("OK")
