# **本家の受け入れ仕様から起こした検査**(2026-08-13)。
#
# python-docx は features/*.feature に「利用者から見た約束」を Gherkin で
# 書いている。その約束をうちの口で確かめる — 出所は NOTICE.md に書いた。
# 写したのは**約束**であって、向こうのコードではない。
#
# ここは test_doc.py(うちの口が動くか)とは役目が違う。**本家の定義に
# 照らして合っているか**だけを見る。合っていない所は台帳に穴として残す
# (docs/pysheet-gokan.ja.md の「本家の定義とずれている所」)。
#
# 手で回すなら:
#   .venv/bin/python pysheet/test_shiyou.py
import sys

from officework import doc as od


def check(cond, msg):
    if not cond:
        print(f"NG: {msg}", file=sys.stderr)
        sys.exit(1)


def raises(exc, f, msg):
    try:
        f()
    except exc:
        return
    except Exception as e:
        check(False, f"{msg}(別の例外: {type(e).__name__}: {e})")
    check(False, f"{msg}(何も起きなかった)")


# ── 段落 ──────────────────────────────────────────────────────
# doc-add-paragraph: 字も様式も省ける。字を渡せばその字を持つ
d = od.Doc()
p0 = d.add_paragraph()
check(p0.text == "", f"空の段落に字がある: {p0.text!r}")
p1 = d.add_paragraph("いろは")
check(p1.text == "いろは", "渡した字が段落に入らない")
check(d.paragraphs[-1].text == "いろは", "足した段落が末尾に来ない")

# doc-add-paragraph: 様式は名前でも様式の物でも渡せる
p2 = d.add_paragraph("見出しにする", style="heading1")
check(p2.style == "heading1", f"名前で渡した様式が付かない: {p2.style}")
p3 = d.add_paragraph("これも", style=d.styles["heading2"])
check(p3.style == "heading2", f"様式の物で渡せない: {p3.style}")

# par-clear-paragraph: 字は消えるが**段落の性質は残る**
pc = d.add_paragraph("消される字", style="heading1")
pc.align = "center"
check(pc.clear() is pc, "clear が自分を返さない(本家と同じ定義)")
check(pc.text == "", "clear で字が消えない")
check(pc.style == "heading1" and pc.align == "center", "clear で段落の性質が飛んだ")

# par-set-text: 字を入れ替えても様式は残る
ps = d.add_paragraph("もとの字", style="heading2")
ps.text = "あとの字"
check(ps.text == "あとの字" and ps.style == "heading2", "字の入れ替えで様式が飛んだ")

# par-insert-paragraph: 前に差し込むと1つ増え、その位置に入る
n = len(d.paragraphs)
pi = ps.insert_paragraph_before("差し込んだ", style="heading3")
check(len(d.paragraphs) == n + 1, "差し込んでも段落が増えない")
i = [x.text for x in d.paragraphs].index("差し込んだ")
check(d.paragraphs[i + 1].text == "あとの字", "差し込みが指した段落の前に入っていない")
check(pi.style == "heading3", "差し込んだ段落の様式が付かない")

# doc-add-heading: 既定は1段目。level を渡せばその段
h1 = d.add_heading("見出し")
check(h1.style == "heading1", f"add_heading の既定が1段目でない: {h1.style}")
check(h1.text == "見出し", "見出しの字が入らない")
check(d.add_heading("2段目", 2).style == "heading2", "level=2 が2段目でない")
check(d.add_heading("3段目", 3).style == "heading3", "level=3 が3段目でない")
# **うちの見出しは3段まで**(模型の粒度)。本家の 0=Title・4〜9 は正直に断る
raises(ValueError, lambda: d.add_heading("題", 0), "level=0 を黙って受けている")
raises(ValueError, lambda: d.add_heading("題", 4), "level=4 を黙って受けている")
raises(ValueError, lambda: d.add_heading("題", 10), "範囲の外を黙って受けている")

# doc-add-page-break: 改ページだけの段落が末尾に付く
before = len(d.paragraphs)
d.add_page_break()
check(len(d.paragraphs) == before + 1, "改ページで段落が増えない")
check(d.paragraphs[-1].paragraph_format.page_break_before is True,
      "改ページの段落に page_break_before が立っていない")

# ── run ───────────────────────────────────────────────────────
# par-add-run: 字を渡せばその字を持つ run が末尾に付く
pr = d.add_paragraph("あ")
r = pr.add_run("い")
check(r.text == "い", "add_run に渡した字が入らない")
check(pr.text == "あい", "run を足しても段落の字に出てこない")

# run の書式は**全部の口が書ける**(台帳の約束)。size_pt は包み(_doc.py)の
# setter だけが漏れていて、Rust 側にはあるのに Python から書けなかった
# (2026-08-14 に発見)— 口ごとに縛って、片方だけの漏れを捕まえる
r.size_pt = 14
check(r.size_pt == 14.0, f"size_pt が書けない: {r.size_pt}")
r.size_pt = None
check(r.size_pt is None, "size_pt の指定を外せない(None = 文書の既定)")

# run-clear-run: 字は消えるが**書式は残る**
r.bold = True
r.italic = True
check(r.clear() is r, "run.clear が自分を返さない")
check(r.text == "", "run.clear で字が消えない")
check(r.bold is True and r.italic is True, "run.clear で書式が飛んだ")

# run-add-content / txt-add-break: 改行・タブは run の中身として順に並ぶ
rb = d.add_paragraph("").add_run("あ")
rb.add_break()
rb.add_text("い")
rb.add_tab()
rb.add_text("う")
kinds = [type(x).__name__ if not isinstance(x, str) else "str"
         for x in rb.iter_inner_content()]
check(kinds == ["str", "Break", "str", "Tab", "str"],
      f"run の中身が順に返らない: {kinds}")

# run-char-style: 文字スタイルは名前でも様式の物でも
d.styles.add_style("強い", "character")
rs = d.add_paragraph("").add_run("字")
rs.style = "強い"
check(rs.style == "強い", f"文字スタイルが付かない: {rs.style}")
# 無い名前は**黙って作らない**(add_style で作ってから、が家の作法)
raises(Exception, lambda: setattr(rs, "style", "無い様式"), "無い様式を黙って受けている")

# ── 表 ────────────────────────────────────────────────────────
# doc-add-table: 行数・列数のとおりに出来る
t = d.add_table(rows=2, cols=3)
check(t.shape == (2, 3), f"表の形が違う: {t.shape}")
check(len(t.rows) == 2 and len(t.columns) == 3, "行・列の数が合わない")

# tbl-cell-text / tbl-cell-access: セルの字は代入でき、行・列から引ける
t.cell(0, 0).text = "左上"
t.cell(1, 2).text = "右下"
check(t.cell(0, 0).text == "左上", "セルに入れた字が読めない")
check(t.rows[0].cells[0].text == "左上", "行から引いたセルが食い違う")
check(t.columns[2].cells[1].text == "右下", "列から引いたセルが食い違う")
check(t[1][2].text == "右下", "添字で引いたセルが食い違う")

# tbl-add-row-or-col: 足した行は同じ列数を持つ
t.add_row()
check(t.shape == (3, 3), f"行を足しても形が変わらない: {t.shape}")
check(len(t.rows[-1].cells) == 3, "足した行の列数が違う")
# 本家は add_column(width) と幅を要るが、**等分の表に1列だけ幅は形が
# 決まらない** — 正直に断り、幅なしで足す道を案内する(家の作法)
raises(ValueError, lambda: t.add_column(od.Mm(30)),
       "等分の表に幅つきの列を黙って足している")
t.add_column()
check(t.shape == (3, 4), f"列を足しても形が変わらない: {t.shape}")
check(len(t.columns[-1].cells) == 3, "足した列の行数が違う")

# tbl-props: autofit は明示が無ければ True(本家の「no explicit → autofit」)
check(t.autofit is True, f"autofit の既定が True でない: {t.autofit}")
t.autofit = False
check(t.autofit is False, "autofit を False にできない")

# ── 断ると決めた所(黙って落とさない)──────────────────────────
# cmt-* : 本家は run から run までを範囲にするが、うちのコメントは段落単位
rm = d.add_paragraph("字").add_run("あ")
raises(NotImplementedError, lambda: rm.mark_comment_range(rm, 1),
       "範囲コメントを黙って受けている")
# txt-parfmt-props: 段落の前後の余白は模型に無い
pf = d.add_paragraph("字").paragraph_format
raises(NotImplementedError, lambda: setattr(pf, "space_before", 100),
       "段落前の余白を黙って受けている")
raises(NotImplementedError, lambda: setattr(pf, "space_after", 100),
       "段落後の余白を黙って受けている")

# ── 無指定は無指定のまま往復する(2026-08-13 に塞いだ穴)──────
# 指定の無い文字の大きさが往復で 10.5pt に焼き付いていた(w:sz を必ず
# 書く + Run.size_pt が f32 で無指定を持てない)。Run.size_pt を
# Option にして根治 — 本家の font.size が None を返すのと同じ約束。
try:
    import docx as _honke
except ImportError:
    _honke = None

if _honke is not None:
    import os
    import tempfile

    with tempfile.TemporaryDirectory() as t:
        moto = os.path.join(t, "moto.docx")
        ato = os.path.join(t, "ato.docx")
        hd = _honke.Document()
        hd.add_paragraph("大きさを指定していない字")
        hd.save(moto)
        check(_honke.Document(moto).paragraphs[0].runs[0].font.size is None,
              "本家が作った時点で大きさが入っている(前提が崩れた)")
        od.Doc.open(moto).save(ato)
        ima = _honke.Document(ato).paragraphs[0].runs[0].font.size
        check(ima is None,
              f"指定の無い大きさが往復で焼き付いた(本家の読み: {ima!r}。"
              "None のままが正 — 2026-08-13 に塞いだ穴が開き直している)")
        # うちの口でも同じ約束(size_pt は None = 指定なし)
        check(od.Doc.open(moto).paragraphs[0].runs[0].size_pt is None,
              "無指定の run の size_pt が None でない")

print("OK")
