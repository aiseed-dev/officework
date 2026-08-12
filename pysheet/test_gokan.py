# 互換層の適合検査 — **書きは定義どおり動作するか**(2026-08-12 発注者確定の合否線)。
#
# 本家(openpyxl / python-docx)が .venv に居れば、同じ手順を両方で動かして
# **結果そのものを突き合わせる**。居なければその節は飛ばす(無いのに失敗と言わない)。
# xlwings は Excel が無いと動かないので、参照の算術は文書の定義値と照合する。
#
# 手で回すなら:
#   .venv/bin/python pysheet/test_gokan.py
import os
import sys
import tempfile

from officework import sheet as office_sheet


def check(cond, msg):
    if not cond:
        print(f"NG: {msg}", file=sys.stderr)
        sys.exit(1)


# =============================================================== openpyxl の口
try:
    import openpyxl
except ImportError:
    openpyxl = None
    print("openpyxl が無いので突き合わせは飛ばした", file=sys.stderr)

b = office_sheet.Book()
s = b[0]

# --- 空のシートの端(openpyxl の定義: 空でも 1 と "A1:A1")---------------------
check(s.max_row == 1 and s.max_column == 1, f"空の max が 1 でない: {s.max_row},{s.max_column}")
check(s.min_row == 1 and s.min_column == 1, "空の min が 1 でない")
check(s.dimensions == "A1:A1", f"空の dimensions: {s.dimensions}")

# --- append: 使われている範囲の次の行へ。dict は列を選んで書く ------------------
s.append([1, 2])
s.append({"B": 9})          # 列の字で
s.append({3: "う"})         # 列の番号(1起点)でも
check(s["A1"] == 1 and s["B1"] == 2, f"append の1行目: {s['A1']},{s['B1']}")
check(s["B2"] == 9, f"append の dict(字): {s['B2']}")
check(s["C3"] == "う", f"append の dict(番号): {s['C3']}")
check(s.dimensions == "A1:C3", f"append 後の dimensions: {s.dimensions}")

# --- cell(row, column, value=): 書いて、札(座標)が openpyxl と同じ形 ----------
c = s.cell(row=2, column=3, value=7)
check(s["C2"] == 7, "cell(value=) が書けていない")
check(c.coordinate == "C2" and c.column_letter == "C" and c.col_idx == 3,
      f"Cell の札: {c.coordinate},{c.column_letter},{c.col_idx}")
check(c.data_type == "n", f"数の data_type: {c.data_type}")
check(c.offset(row=1, column=-2).coordinate == "A3", "offset の算術")
check(c.parent is s, "Cell.parent がシートに戻らない")
s["D1"] = "=A1+B1"
check(s.cell(row=1, column=4).data_type == "f", "式の data_type が 'f' でない")
check(s.cell(row=3, column=3).data_type == "s", "字の data_type が 's' でない")

# --- iter_rows / iter_cols / rows / columns ------------------------------------
got = [[cc.coordinate for cc in row] for row in s.iter_rows(min_row=1, max_row=2, max_col=2)]
check(got == [["A1", "B1"], ["A2", "B2"]], f"iter_rows の座標: {got}")
got = list(s.iter_rows(min_row=1, max_row=1, max_col=2, values_only=True))
check(got == [(1, 2)], f"iter_rows(values_only): {got}")
got = [[cc.coordinate for cc in col] for col in s.iter_cols(min_col=2, max_col=3, max_row=2)]
check(got == [["B1", "B2"], ["C1", "C2"]], f"iter_cols の座標: {got}")
check(len(list(s.rows)) == s.max_row, "rows が max_row と食い違う")
check(len(list(s.columns)) == s.max_column, "columns が max_column と食い違う")

# --- Workbook の口 --------------------------------------------------------------
check(b.sheetnames == b.sheet_names, "sheetnames が sheet_names と食い違う")
ws2 = b.create_sheet()
check(ws2.title in b.sheetnames, "create_sheet の既定の名前が一覧に無い")
check(b.index(ws2) == len(b) - 1, "index(ws) が末尾を指さない")
check(ws2.parent is b, "parent がブックに戻らない")
check([w.title for w in b.worksheets] == b.sheetnames, "worksheets の並び")
check(b.active.title == b.sheetnames[0], "active が先頭でない")
check(ws2.title in b and "居ない名前" not in b, "in(__contains__)が効かない")
b.close()  # 何もしないが、呼べること

# --- insert_rows / delete_rows / insert_cols / delete_cols(amount つき)--------
# openpyxl と同じ手順を並べて動かし、**盤面の値が同じになるか**を見る
if openpyxl is not None:
    def grid(rows, cols, at):
        return [[at(i, j) for j in range(1, cols + 1)] for i in range(1, rows + 1)]

    wb_o = openpyxl.Workbook()
    ws_o = wb_o.active
    b_j = office_sheet.Book()
    ws_j = b_j[0]
    for i in range(1, 4):
        for j in range(1, 4):
            ws_o.cell(row=i, column=j, value=i * 10 + j)
            ws_j.cell(row=i, column=j, value=i * 10 + j)
    for ws in (ws_o, ws_j):
        ws.insert_rows(2, amount=2)
        ws.delete_rows(1)
        ws.insert_cols(2, amount=1)
        ws.delete_cols(4, amount=2)
    g_o = grid(4, 2, lambda i, j: ws_o.cell(row=i, column=j).value)
    g_j = grid(4, 2, lambda i, j: ws_j.cell(row=i, column=j).value)
    check(g_o == g_j, f"行・列の出し入れが openpyxl と食い違う:\n  本家 {g_o}\n  うち {g_j}")

    # --- 定数(19個)は openpyxl の実物と同じ値 --------------------------------
    from openpyxl.worksheet.worksheet import Worksheet as _W
    for n in dir(_W):
        if n.split("_")[0] in ("BREAK", "ORIENTATION", "PAPERSIZE", "SHEETSTATE"):
            check(getattr(office_sheet.Sheet, n) == getattr(_W, n),
                  f"定数 {n} が openpyxl と違う")

    # --- 書いた物を本家が読めるか(定義どおりの何よりの証拠)--------------------
    with tempfile.TemporaryDirectory() as t:
        out = os.path.join(t, "gokan.xlsx")
        b2 = office_sheet.Book()
        s2 = b2[0]
        s2.append(["品名", "数", "単価", "金額"])
        s2.append(["ザボガードF", 4, 125000, "=B2*C2"])
        b2.save(out)
        r = openpyxl.load_workbook(out)
        rs = r.active
        check(rs["A1"].value == "品名" and rs["B2"].value == 4,
              "うちが書いた値を openpyxl が読めない")
        check(rs["D2"].value == "=B2*C2", "うちが書いた式を openpyxl が読めない")
        rv = openpyxl.load_workbook(out, data_only=True).active
        check(rv["D2"].value == 500000,
              f"うちが書き込んだ計算済みの値(openpyxl 自身は作れない物): {rv['D2'].value}")

        # 逆向き: 本家が書いた物をうちが読めるか
        out2 = os.path.join(t, "opx.xlsx")
        wb3 = openpyxl.Workbook()
        ws3 = wb3.active
        ws3.append([1, "あ", True])
        wb3.save(out2)
        b3 = office_sheet.Book.open(out2)
        s3 = b3[0]
        check(s3["A1"] == 1 and s3["B1"] == "あ" and s3["C1"] is True,
              "openpyxl が書いた物をうちが読めない")

# (第1歩では title の代入と create_sheet(index=) は「正直に断る」だったが、
#  第2歩でエンジンに書き口が入った — 下の第2歩の節で本式に検査する)

# --- 書式の書き: うちが書いた書式を本家が読める(定義どおりの何よりの証拠)------
if openpyxl is not None:
    with tempfile.TemporaryDirectory() as t:
        out = os.path.join(t, "fmt.xlsx")
        bf = office_sheet.Book()
        sf = bf[0]
        c = sf.cell(1, 1, value="題")
        c.font = office_sheet.Font(bold=True, size=14, color="FF0000")
        c.border = office_sheet.Border(
            top=office_sheet.Side("thin"),
            bottom=office_sheet.Side("double", office_sheet.Color("0070C0")),
        )
        c.fill = office_sheet.PatternFill("solid", fgColor="FFFF00")
        c.alignment = office_sheet.Alignment(
            horizontal="center", vertical="center", wrap_text=True
        )
        sf["B1"] = 45000
        sf.cell(1, 2).number_format = "yyyy/mm/dd"
        check(sf.cell(1, 2).is_date, "日付の表示形式なのに is_date が False")
        check(not sf.cell(1, 1).is_date, "字のセルまで is_date が True")
        bf.save(out)

        rc = openpyxl.load_workbook(out).active["A1"]
        check(rc.font.bold and rc.font.size == 14, f"font が本家で読めない: {rc.font}")
        check((rc.font.color.rgb or "").endswith("FF0000"), f"文字色: {rc.font.color}")
        check(rc.border.top.style == "thin" and rc.border.bottom.style == "double",
              f"罫線: {rc.border.top.style},{rc.border.bottom.style}")
        check((rc.border.bottom.color.rgb or "").endswith("0070C0"),
              f"罫線の色: {rc.border.bottom.color}")
        check(rc.fill.patternType == "solid"
              and (rc.fill.fgColor.rgb or "").endswith("FFFF00"),
              f"塗り: {rc.fill.patternType},{rc.fill.fgColor}")
        check(rc.alignment.horizontal == "center"
              and rc.alignment.vertical == "center" and rc.alignment.wrap_text,
              f"揃え: {rc.alignment}")
        rb = openpyxl.load_workbook(out).active["B1"]
        check(rb.number_format == "yyyy/mm/dd", f"表示形式: {rb.number_format}")

        # 逆向き: 本家が書いた書式をうちが読める
        out2 = os.path.join(t, "fmt_opx.xlsx")
        from openpyxl.styles import (Alignment as OAlign, Border as OBorder,
                                     Font as OFont, PatternFill as OFill,
                                     Side as OSide)
        wb4 = openpyxl.Workbook()
        ws4 = wb4.active
        oc = ws4["A1"]
        oc.value = "題"
        oc.font = OFont(bold=True, size=12, color="00B050")
        oc.border = OBorder(left=OSide(style="medium"))
        oc.fill = OFill("solid", fgColor="D9D9D9")
        oc.alignment = OAlign(horizontal="right", wrap_text=True)
        oc.number_format = "#,##0"
        wb4.save(out2)

        b5 = office_sheet.Book.open(out2)
        d = b5[0].fmt("A1")
        check(d.get("bold") and d.get("size") == 12.0, f"本家の font がうちで読めない: {d}")
        check(d.get("color") == "00B050", f"本家の文字色: {d.get('color')}")
        check(d.get("border_left", (None,))[0] == "medium", f"本家の罫線: {d}")
        check(d.get("fill") == "D9D9D9", f"本家の塗り: {d.get('fill')}")
        check(d.get("horizontal") == "right" and d.get("wrap"), f"本家の揃え: {d}")
        check(d.get("number_format") == "#,##0", f"本家の表示形式: {d}")
        # openpyxl の実物の入れ物をそのまま代入しても効く(属性名で受ける)
        c5 = b5[0].cell(2, 1)
        c5.font = OFont(italic=True)
        check(b5[0].fmt("A2").get("italic"), "openpyxl の Font の代入が効かない")

# ================================================== xlwings の口(参照の算術)
# 橋は動いているアプリが要るので、ソケットに出ない算術だけを定義値と照合する
from officework import calc as xw

r = xw.Range("B2:D5")
check(r.address == "$B$2:$D$5", f"address: {r.address}")
check(r.get_address(False, False) == "B2:D5", "get_address(相対)")
check(r.row == 2 and r.column == 2, f"row,column: {r.row},{r.column}")
check(r.shape == (4, 3) and r.size == 12 and r.count == 12 and len(r) == 12,
      f"shape,size: {r.shape},{r.size}")
check(len(r.rows) == 4 and r.rows[0]._a1() == "B2:D2", "rows の刻み")
check(len(r.columns) == 3 and r.columns[2]._a1() == "D2:D5", "columns の刻み")
check(r.offset(1, 2)._a1() == "D3:F6", f"offset: {r.offset(1, 2)._a1()}")
check(r.resize(2, 2)._a1() == "B2:C3", f"resize: {r.resize(2, 2)._a1()}")
check(r.resize(row_size=1)._a1() == "B2:D2", "resize(片方だけ)")
check(r.last_cell._a1() == "D5", f"last_cell: {r.last_cell._a1()}")
one = xw.Range("A1")
check(one.address == "$A$1" and one.shape == (1, 1), "1マスの算術")
sh = xw.Sheet("見積")
check(sh.cells._a1() == "A1:XFD1048576", "Sheet.cells が全マスでない")
check(sh["B2"]._a1() == "B2", "Sheet の添字")

# =============================================== python-docx の口(表と段落)
try:
    import docx as pydocx
except ImportError:
    pydocx = None
    print("python-docx が無いので突き合わせは飛ばした", file=sys.stderr)

from officework import doc as office_doc

if pydocx is not None:
    with tempfile.TemporaryDirectory() as t:
        # 本家で作った文書を、うちで読んで**同じ添字が同じセルを指す**か
        src = os.path.join(t, "hon.docx")
        d_o = pydocx.Document()
        d_o.add_paragraph("最初の段落")
        tb = d_o.add_table(rows=3, cols=2)
        for i in range(3):
            for j in range(2):
                tb.cell(i, j).text = f"{i}-{j}"
        d_o.save(src)

        d_j = office_doc.Doc.open(src)
        t_j = d_j.tables[0]
        t_o = pydocx.Document(src).tables[0]
        for i in range(3):
            for j in range(2):
                check(t_j.cell(i, j).text == t_o.cell(i, j).text,
                      f"cell({i},{j}) が本家と食い違う")
        check([c.text for c in t_j.row_cells(1)] == [c.text for c in t_o.row_cells(1)],
              "row_cells が本家と食い違う")
        check([c.text for c in t_j.column_cells(1)] == [c.text for c in t_o.column_cells(1)],
              "column_cells が本家と食い違う")
        check(len(t_j.columns) == len(t_o.columns), "columns の数")
        check([c.text for c in t_j.columns[1].cells] == [c.text for c in t_o.columns[1].cells],
              "columns[j].cells が本家と食い違う")

        # clear: 字は消え、段落の性質は残り、自分が返る(本家と同じ定義)
        p_o = pydocx.Document(src).paragraphs[0]
        ret_o = p_o.clear()
        p_j = d_j[0]
        ret_j = p_j.clear()
        check(p_j.text == "" and p_o.text == "", "clear で字が消えない")
        check(ret_j is p_j and ret_o is p_o, "clear が自分を返さない")

        # iter_inner_content: run が順に出る
        d_j2 = office_doc.Doc.open(src)
        p = d_j2[0]
        check([r.text for r in p.iter_inner_content()] == [r.text for r in p.runs],
              "iter_inner_content が runs と食い違う")

        # 書きの往復: うちがセルへ書いた物を本家が読めるか
        t_j2 = d_j2.tables[0]
        t_j2.cell(1, 1).text = "書き換えた"
        out = os.path.join(t, "kaki.docx")
        d_j2.save(out)
        back = pydocx.Document(out)
        check(back.tables[0].cell(1, 1).text == "書き換えた",
              "うちが書いたセルを本家が読めない")

        # font の両対応: 字の比べも .name も通る
        rr = d_j2[0].runs if d_j2[0].runs else None
        if rr:
            f = rr[0].font
            check(isinstance(f, str) and (f.name is None or isinstance(f.name, str)),
                  "font の両対応(str と .name)が崩れている")

        # --- 段落の書式: add_heading・style の書き・paragraph_format ----------
        # うちが書いた物を本家が読める(定義どおりの何よりの証拠)
        d_h = office_doc.Doc()
        h = d_h.add_heading("第1章 概要", level=1)
        check(h.style == "heading1", f"add_heading の役目: {h.style}")
        p_f = d_h.add_paragraph("本文です")
        p_f.paragraph_format.alignment = "center"
        p_f.paragraph_format.line_spacing = 1.5
        p_f.paragraph_format.page_break_before = True
        p2 = d_h.add_paragraph("次の段落")
        p2.style = "Heading 2"          # python-docx の名前でも受ける
        check(p2.style == "heading2", f"style の書き(本家の名前): {p2.style}")
        out_h = os.path.join(t, "heading.docx")
        d_h.save(out_h)

        from docx.enum.text import WD_ALIGN_PARAGRAPH
        back_h = pydocx.Document(out_h)
        check(back_h.paragraphs[0].style.name == "Heading 1",
              f"うちの見出しを本家が読めない: {back_h.paragraphs[0].style.name}")
        bf = back_h.paragraphs[1].paragraph_format
        check(bf.alignment == WD_ALIGN_PARAGRAPH.CENTER,
              f"うちの寄せを本家が読めない: {bf.alignment}")
        check(bf.line_spacing == 1.5, f"うちの行間を本家が読めない: {bf.line_spacing}")
        check(bf.page_break_before, "うちの改ページ前を本家が読めない")
        check(back_h.paragraphs[2].style.name == "Heading 2",
              "style の書きが保存で消えた")

        # 逆向き: 本家が書いた物をうちが読める
        d_o2 = pydocx.Document()
        d_o2.add_heading("題", level=2)
        po = d_o2.add_paragraph("中央寄せ")
        po.paragraph_format.alignment = WD_ALIGN_PARAGRAPH.CENTER
        po.paragraph_format.line_spacing = 2.0
        out_o = os.path.join(t, "heading_opx.docx")
        d_o2.save(out_o)
        d_r = office_doc.Doc.open(out_o)
        check(d_r[0].style == "heading2", f"本家の見出しがうちで読めない: {d_r[0].style}")
        check(d_r[1].paragraph_format.alignment == "center",
              f"本家の寄せ: {d_r[1].paragraph_format.alignment}")
        check(d_r[1].paragraph_format.line_spacing == 2.0,
              f"本家の行間: {d_r[1].paragraph_format.line_spacing}")
        # 本家の enum をそのまま代入しても効く
        d_r[1].alignment = WD_ALIGN_PARAGRAPH.RIGHT
        check(d_r[1].align == "right", "本家の enum の代入が効かない")
        # 模型に無い物は黙って捨てない
        try:
            d_r[1].paragraph_format.space_before = 12
            check(False, "space_before が黙って通った")
        except NotImplementedError:
            pass
        try:
            d_h.add_heading("題", level=0)
            check(False, "level=0(Title)が黙って通った")
        except ValueError:
            pass

        # --- 表の書式: style(名前だけ運ぶ)・alignment・autofit ---------------
        from docx.enum.table import WD_TABLE_ALIGNMENT
        d_t = pydocx.Document()
        tb2 = d_t.add_table(rows=2, cols=2, style="Table Grid")
        tb2.alignment = WD_TABLE_ALIGNMENT.CENTER
        tb2.autofit = False
        out_t = os.path.join(t, "tbl.docx")
        d_t.save(out_t)

        d_tr = office_doc.Doc.open(out_t)
        t_r = d_tr.tables[0]
        check(t_r.style == "TableGrid", f"本家の表スタイルがうちで読めない: {t_r.style}")
        check(t_r.alignment == "center", f"本家の表の置き方: {t_r.alignment}")
        check(t_r.autofit is False, f"本家の autofit: {t_r.autofit}")

        # うちが書き替えた物を本家が読める(スタイル定義は原本の物が持ち越される)
        t_r.alignment = WD_TABLE_ALIGNMENT.RIGHT  # 本家の enum をそのまま
        t_r.autofit = True
        out_t2 = os.path.join(t, "tbl2.docx")
        d_tr.save(out_t2)
        back_t = pydocx.Document(out_t2).tables[0]
        check(back_t.style.name == "Table Grid",
              f"うちが運んだスタイル名を本家が読めない: {back_t.style.name}")
        check(back_t.alignment == WD_TABLE_ALIGNMENT.RIGHT,
              f"うちの表の置き方を本家が読めない: {back_t.alignment}")
        check(back_t.autofit, "うちの autofit を本家が読めない")

        # --- run の手: add_run・性質ごとの書き・add_text・clear ----------------
        d_run = office_doc.Doc()
        pr = d_run.add_paragraph("請求先: ")
        r2 = pr.add_run("株式会社甲")
        r2.bold = True
        r2.font.size = 12
        r2.add_text(" 御中")
        check(pr.text == "請求先: 株式会社甲 御中", f"add_run/add_text: {pr.text}")
        check(pr.runs[1].bold and pr.runs[1].size_pt == 12, "run の性質の書き")
        d_run.add_page_break()
        p_last = d_run.add_paragraph("次の頁")
        out_r = os.path.join(t, "runs.docx")
        d_run.save(out_r)

        back_r = pydocx.Document(out_r)
        rr2 = back_r.paragraphs[0].runs
        check(len(rr2) == 2 and rr2[1].bold and rr2[1].font.size.pt == 12,
              f"うちの run の書式を本家が読めない: {[(r.text, r.bold) for r in rr2]}")
        check(back_r.paragraphs[0].text == "請求先: 株式会社甲 御中",
              "run の字が本家で崩れる")
        # 改ページはうちの流儀(page_break_before)— 本家でもその形で読める
        check(back_r.paragraphs[1].paragraph_format.page_break_before,
              "add_page_break が本家で読めない")

        # 本家の run の作法(clear が自分を返す・add_text が書式を保つ)と同じか
        d_o3 = pydocx.Document()
        po3 = d_o3.add_paragraph("あ")
        ro3 = po3.add_run("い")
        ro3.bold = True
        ro3.add_text("う")
        check(po3.text == "あいう" and po3.runs[1].bold, "(本家の前提の確認)")
        r_mine = office_doc.Doc().add_paragraph("あ").add_run("い")
        r_mine.bold = True
        r_mine.add_text("う")
        check(r_mine.text == "いう" and r_mine.bold, "うちの add_text が書式を落とす")
        check(r_mine.clear() is r_mine and r_mine.text == "" and r_mine.bold,
              "clear が自分を返さない・書式まで消える")

        # --- 文書の順・途中に差す・文書の情報・画像 ----------------------------
        d_mix = office_doc.Doc()
        d_mix.add_paragraph("前")
        d_mix.add_table(1, 1)
        after = d_mix.add_paragraph("後")
        kinds = [type(x).__name__ for x in d_mix.iter_inner_content()]
        check(kinds == ["Paragraph", "Table", "Paragraph"],
              f"iter_inner_content の順: {kinds}")
        after.insert_paragraph_before("間")
        check([p.text for p in d_mix.paragraphs] == ["前", "間", "後"],
              f"insert_paragraph_before: {[p.text for p in d_mix.paragraphs]}")

        d_mix.core_properties.author = "日本不燃 太郎"
        d_mix.core_properties.title = "見積書"
        # 画像(2×2 の最小 PNG)を径路の代わりに bytes で
        png = (b"\x89PNG\r\n\x1a\n" +
               b"\x00\x00\x00\rIHDR\x00\x00\x00\x02\x00\x00\x00\x02"
               b"\x08\x02\x00\x00\x00\xfd\xd4\x9as" +
               b"\x00\x00\x00\x0cIDATx\x9cc\xf8\xff\xff?\x00\x05\xfe\x02\xfe"
               b"\xa75\x81\x84\x00\x00\x00\x00IEND\xaeB`\x82")
        d_mix.add_picture(png, width=30)  # mm。縦横比を保って 30×30
        out_m = os.path.join(t, "mix.docx")
        d_mix.save(out_m)

        back_m = pydocx.Document(out_m)
        check(back_m.core_properties.author == "日本不燃 太郎"
              and back_m.core_properties.title == "見積書",
              f"文書の情報を本家が読めない: {back_m.core_properties.author}")
        check(len(back_m.inline_shapes) == 1, "うちの画像を本家が読めない")
        check(round(back_m.inline_shapes[0].width.mm) == 30,
              f"画像の大きさ: {back_m.inline_shapes[0].width.mm}")
        # 逆向き: 本家の文書の情報をうちが読める
        d_o4 = pydocx.Document()
        d_o4.core_properties.author = "甲"
        d_o4.core_properties.comments = "控え"
        out_o4 = os.path.join(t, "props.docx")
        d_o4.save(out_o4)
        d_r4 = office_doc.Doc.open(out_o4)
        check(d_r4.core_properties.author == "甲"
              and d_r4.core_properties.comments == "控え",
              "本家の文書の情報がうちで読めない")

        # --- inline_shapes(画像の読みの対)と コメント -------------------------
        d_m2 = office_doc.Doc.open(out_m)  # さっき画像を入れた文書
        shp = d_m2.inline_shapes
        check(len(shp) == 1 and round(shp[0].width.mm) == 30,
              f"inline_shapes: {shp}")
        p0 = d_m2[0]
        p0.add_comment("この行を確認", author="乙")
        check(d_m2.comments[0].text == "この行を確認"
              and d_m2.comments[0].author == "乙"
              and d_m2.comments[0].paragraph.text == p0.text,
              f"コメントの読み書き: {d_m2.comments}")
        out_c = os.path.join(t, "cmt.docx")
        d_m2.save(out_c)
        d_c = office_doc.Doc.open(out_c)
        check(d_c.comments and d_c.comments[0].text == "この行を確認",
              "コメントが保存で消えた")
        # 本家(1.2 以降)がコメント API を持つなら突き合わせる
        back_c = pydocx.Document(out_c)
        if hasattr(back_c, "comments"):
            check(any(c.text == "この行を確認" for c in back_c.comments),
                  f"うちのコメントを本家が読めない: {[c.text for c in back_c.comments]}")

# ==================== 第2歩(足すの背骨): 結合・固定枠・改名・複製・削除・並べ替え
b = office_sheet.Book()
s = b[0]

# 結合: 家の作法(アプリと同じ)— 左上以外の中身は消え、空の左上へは最初の中身が移る
s["B1"] = "題"
s["C2"] = 9
s.merge_cells("A1:C2")
check(s.merged_cell_ranges == ["A1:C2"], f"結合が台帳に載らない: {s.merged_cell_ranges}")
check(s["A1"] == "題", "空だった左上へ最初の中身が移っていない")
check(s["B1"] is None and s["C2"] is None, "呑まれた中身が消えていない")

# 解除: openpyxl と同じ定義 — その範囲そのものが結合でなければ ValueError
try:
    s.unmerge_cells("A1:B1")
    check(False, "結合でない範囲の解除が黙って通った")
except ValueError:
    pass
s.unmerge_cells("A1:C2")
check(s.merged_cell_ranges == [], "解除できていない")

# openpyxl の数字指定でも
s.merge_cells(start_row=1, start_column=1, end_row=2, end_column=2)
check(s.merged_cell_ranges == ["A1:B2"], "数字指定の結合")
s.unmerge_cells(start_row=1, start_column=1, end_row=2, end_column=2)

# 固定枠: A1 形式(openpyxl と同じ定義)
s.freeze_panes = "B2"
check(s.freeze_panes == "B2", f"固定枠: {s.freeze_panes}")
s.freeze_panes = "A1"  # A1 は「固定なし」
check(s.freeze_panes is None, "A1 で固定が解けない")
s.freeze_panes = "A3"  # 上2行だけ
check(s.freeze_panes == "A3", "行だけの固定")
s.freeze_panes = None

# 改名: 式の参照と名前の定義が追随する(openpyxl は追随しない — うちの上位分)
b2 = office_sheet.Book()
b2[0]["A1"] = 42
w2 = b2.create_sheet("集計")
w2["A1"] = "=Sheet1!A1*2"
check(w2["A1"] == 84, "他のシートへの式")
b2[0].title = "元データ"
check(b2.sheetnames[0] == "元データ", "改名が一覧に出ない")
check(w2.formula("A1") == "=元データ!A1*2", f"改名に式が追随しない: {w2.formula('A1')}")
check(w2["A1"] == 84, "改名後の再計算")
try:
    w2.title = "元データ"
    check(False, "同じ名前への改名が通った")
except ValueError:
    pass
try:
    w2.title = "a[b]"
    check(False, "使えない字のシート名が通った")
except ValueError:
    pass

# 複製・削除・並べ替え・途中に差す
w3 = b2.copy_worksheet(b2[0])
check(w3.title == "元データ Copy", f"複製の名前: {w3.title}")
check(w3["A1"] == 42, "複製に中身が写っていない")
w3["A1"] = 1
check(b2[0]["A1"] == 42, "複製が元とつながったまま(独立していない)")
b2.remove(w3)
check("元データ Copy" not in b2.sheetnames, "削除できていない")
head = b2.create_sheet("先頭", 0)
check(b2.sheetnames[0] == "先頭" and head.title == "先頭", f"途中に差す: {b2.sheetnames}")
b2.move_sheet("先頭", offset=1)
check(b2.sheetnames[1] == "先頭", f"move_sheet の相対のずらし: {b2.sheetnames}")

# 最後の1枚は抜けない(正直に断る)
b3 = office_sheet.Book()
try:
    b3.remove(b3[0])
    check(False, "最後の1枚が抜けた")
except ValueError:
    pass

# openpyxl と読み合う: うちの結合・固定枠が本家に見え、本家の物がうちに見える
if openpyxl is not None:
    with tempfile.TemporaryDirectory() as t:
        out = os.path.join(t, "gokan2.xlsx")
        b4 = office_sheet.Book()
        s4 = b4[0]
        s4["A1"] = "見出し"
        s4.merge_cells("A1:C1")
        s4.freeze_panes = "A2"
        b4.save(out)
        rs = openpyxl.load_workbook(out).active
        check([str(r) for r in rs.merged_cells.ranges] == ["A1:C1"],
              f"うちの結合を openpyxl が読めない: {list(rs.merged_cells.ranges)}")
        check(rs.freeze_panes == "A2", f"うちの固定枠を openpyxl が読めない: {rs.freeze_panes}")

        out2 = os.path.join(t, "opx2.xlsx")
        wb5 = openpyxl.Workbook()
        ws5 = wb5.active
        ws5["A1"] = "題"
        ws5.merge_cells("A1:B2")
        ws5.freeze_panes = "B2"
        wb5.save(out2)
        s5 = office_sheet.Book.open(out2)[0]
        check(s5.merged_cell_ranges == ["A1:B2"], "本家の結合をうちが読めない")
        check(s5.freeze_panes == "B2", f"本家の固定枠をうちが読めない: {s5.freeze_panes}")

# ==================== 第3歩: 画像(xlsx)・表の書き(docx)・polars

# --- add_image: 貼って・見えて・保存で xl/media に入り・読み直しで戻る -----------
PNG_1x1 = (
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01"
    b"\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\nIDATx\x9cc\x00\x01"
    b"\x00\x00\x05\x00\x01\r\n-\xb4\x00\x00\x00\x00IEND\xaeB`\x82"
)
b6 = office_sheet.Book()
s6 = b6[0]
s6["A1"] = "グラフの下じき"
s6.add_image(PNG_1x1, "B2")
check(s6.images == [("B2", 1.0, 1.0)], f"貼った画像が見えない: {s6.images}")
s6.add_image(PNG_1x1, "D4", width_px=200, height_px=100)  # 大きさの上書き
with tempfile.TemporaryDirectory() as t:
    out = os.path.join(t, "img.xlsx")
    b6.save(out)
    s7 = office_sheet.Book.open(out)[0]
    check(("B2", 1.0, 1.0) in s7.images and ("D4", 200.0, 100.0) in s7.images,
          f"画像が往復しない: {s7.images}")
    import zipfile as _zf
    with _zf.ZipFile(out) as z:
        media = [n for n in z.namelist() if n.startswith("xl/media/")]
    check(len(media) == 2, f"xl/media に絵が入っていない: {media}")

    # 径路の文字列でも渡せる(matplotlib の savefig の出口をそのまま)
    p = os.path.join(t, "e.png")
    with open(p, "wb") as f:
        f.write(PNG_1x1)
    s6.add_image(p, "F1")
    check(len(s6.images) == 3, "径路の add_image が効かない")

# --- docx: add_table / add_row / add_column ------------------------------------
d_j = office_doc.Doc()
t_j = d_j.add_table(2, 3)
check(t_j.shape == (2, 3), f"add_table の形: {t_j.shape}")
t_j.cell(0, 0).text = "品名"
row = t_j.add_row()
check(t_j.shape == (3, 3) and len(row.cells) == 3, "add_row")
row.cells[0].text = "ザボガードF"
t_j.add_column()
check(t_j.shape == (3, 4), f"add_column: {t_j.shape}")
try:
    d_j.add_table(2, 2, style="Table Grid")
    check(False, "add_table の style が黙って捨てられた")
except NotImplementedError:
    pass

if pydocx is not None:
    with tempfile.TemporaryDirectory() as t:
        out = os.path.join(t, "hyo.docx")
        d_j.save(out)
        back = pydocx.Document(out)
        bt = back.tables[0]
        check(len(bt.rows) == 3, f"うちが組んだ表を本家が読めない: {len(bt.rows)} 行")
        check(len(bt.rows[0].cells) == 4, f"本家の見た列数: {len(bt.rows[0].cells)}")
        check(bt.cell(0, 0).text == "品名" and bt.cell(2, 0).text == "ザボガードF",
              "うちが書いた中身を本家が読めない")

# --- polars: 第一の変換(ソケットに出ない純関数を直接確かめる)------------------
try:
    import polars as pl
except ImportError:
    pl = None
    print("polars が無いので飛ばした", file=sys.stderr)

if pl is not None:
    grid = [["品名", "数"], ["ザボガードF", 4.0], ["ドリル", 2.0]]
    df = xw._grid_to_frame(grid, pl.DataFrame)
    check(df.columns == ["品名", "数"] and df.shape == (2, 2),
          f"polars への変換: {df.columns} {df.shape}")
    check(df["数"].to_list() == [4.0, 2.0], "polars の中身")
    check(xw._to_grid(df) == grid, f"polars からの往復: {xw._to_grid(df)}")
    check(xw._to_grid(pl.Series([1, 2])) == [[1], [2]], "polars の Series")
    # 見出しなし
    df2 = xw._grid_to_frame([[1, 2], [3, 4]], pl.DataFrame, header=False)
    check(df2.shape == (2, 2), "polars header=False")

try:
    import pandas as pd
except ImportError:
    pd = None
    print("pandas が無いので飛ばした", file=sys.stderr)

if pd is not None:
    grid = [["品名", "数"], ["ザボガードF", 4.0]]
    df3 = xw._grid_to_frame(grid, pd.DataFrame)
    check(list(df3.index) == ["ザボガードF"], "pandas の index(従来どおり)")

print("OK")
