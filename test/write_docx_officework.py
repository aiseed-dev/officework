#!/usr/bin/env python3
"""officework エンジンで、test/write_docx_pydocx.py と同じ5枚を作る。

    .venv/bin/python test/write_docx_officework.py [出力先フォルダ(既定 test/out/officework)]

出来上がりの名前は同じ(開催通知.docx など)。python-docx の側と並べて
開いて、目で見比べます。
"""
import sys
from pathlib import Path

from officework import doc
from officework.doc import Doc, Mm

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent / "out" / "officework"
OUT.mkdir(parents=True, exist_ok=True)


def a4(d, landscape=False):
    s = d.sections[0]
    if landscape:
        s.orientation = "landscape"
        s.page_width, s.page_height = Mm(297), Mm(210)
    else:
        s.page_width, s.page_height = Mm(210), Mm(297)
    s.top_margin = s.bottom_margin = Mm(20)
    s.left_margin = s.right_margin = Mm(20)


# ======================================================================
# 1. 開催通知
# ======================================================================
def notice():
    d = Doc()
    a4(d)
    p = d.add_paragraph("2026年8月26日")
    p.alignment = "right"
    p = d.add_paragraph("会員各位")
    p.paragraph_format.space_after = 12
    p = d.add_paragraph("aiseed 事務局")
    p.alignment = "right"

    h = d.add_heading("定例会 開催のお知らせ", level=1)
    h.alignment = "center"

    p = d.add_paragraph()
    p.paragraph_format.first_line_indent = Mm(10)
    p.paragraph_format.line_spacing = 1.5
    p.add_run("下記のとおり定例会を開きます。")
    r = p.add_run("出欠のご返事は 9月5日(金)まで")
    r.bold = True
    r.underline = True
    p.add_run("にお願いします。")

    p = d.add_paragraph("記")
    p.alignment = "center"

    for label, body in [("日時", "9月12日(土) 14:00〜16:00"),
                        ("場所", "工業技術センター 2階 会議室"),
                        ("議題", "新しい道具の紹介と実演")]:
        p = d.add_paragraph()
        p.paragraph_format.left_indent = Mm(20)
        r = p.add_run(label)
        r.bold = True
        p.runs[-1].add_tab()
        p.add_run(body)

    p = d.add_paragraph("以上")
    p.alignment = "right"
    p = d.add_paragraph("問い合わせ: 事務局(内線 123)")
    r = p.runs[0]
    r.size_pt = 9
    r.color = "808080"
    d.core_properties.title = "定例会の開催通知"
    d.core_properties.author = "write_docx_officework"
    d.save(str(OUT / "開催通知.docx"))


# ======================================================================
# 2. 議事録
# ======================================================================
def minutes():
    d = Doc()
    a4(d)
    h = d.add_heading("打ち合わせ議事録", level=1)
    h.alignment = "center"

    t = d.add_table(rows=4, cols=4)
    t.style = "Table Grid"
    t.cell(0, 0).text = "件名"
    a = t.cell(0, 1).merge(t.cell(0, 3))
    a.text = "玄関ドアのカタログ改善"
    t.cell(1, 0).text = "日時"
    t.cell(1, 1).text = "2026-08-26 10:00"
    t.cell(1, 2).text = "場所"
    t.cell(1, 3).text = "第2会議室"
    t.cell(2, 0).text = "出席"
    b = t.cell(2, 1).merge(t.cell(2, 3))
    b.text = "営業部2名・設計1名・事務局1名"
    t.cell(3, 0).text = "記録"
    t.cell(3, 1).text = "事務局"
    for row in t.rows:
        row.height = Mm(8)
        for c in row.cells:
            c.vertical_alignment = "center"
    for c in t.columns[0].cells:
        c.width = Mm(22)
        for p in c.paragraphs:
            for r in p.runs:
                r.bold = True

    d.add_heading("決まったこと", level=2)
    for item in ["型と色を選ぶと合成図が出る形にする",
                 "寸法は物件ごとに mm で控える",
                 "次回までに見本の様式を3枚つくる"]:
        d.add_paragraph(item, style="List Bullet")
    d.add_heading("宿題", level=2)
    for item in ["様式の下書き(営業部)", "防火認定の一覧(設計)", "日程の調整(事務局)"]:
        d.add_paragraph(item, style="List Number")
    d.save(str(OUT / "議事録.docx"))


# ======================================================================
# 3. 操作手順書
# ======================================================================
def manual():
    d = Doc()
    a4(d)
    d.add_heading("月次の締めの手順", level=1)
    d.add_paragraph("毎月1日の朝にやります。10分で終わります。")
    steps = [("台帳を開く", "先月のフォルダの 売上台帳.xlsx を開きます。"),
             ("マクロを押す", None),
             ("印刷する", "できた報告を A4 で1枚印刷し、回覧に載せます。")]
    for title, body in steps:
        d.add_paragraph(title, style="List Number")
        if body:
            p = d.add_paragraph(body)
        else:
            p = d.add_paragraph("マクロの一覧から ")
            r = p.add_run("月次の締め")
            r.font.name = "Courier New"
            p.add_run(" を選んで押します。")
        p.paragraph_format.left_indent = Mm(10)

    logo = Path(__file__).resolve().parent.parent / "packaging/icons/hicolor/128x128/officework.png"
    if logo.exists():
        p = d.add_paragraph()
        p.alignment = "center"
        d.add_picture(str(logo), width=Mm(20))

    d.add_page_break()
    d.add_heading("別紙: 押すボタンの場所", level=2)
    d.add_paragraph("この行は2ページ目の頭にあります(改ページの確かめ)。")
    d.save(str(OUT / "操作手順書.docx"))


# ======================================================================
# 4. 送付状
# ======================================================================
def cover_letter():
    d = Doc()
    a4(d)
    d.header.text = "aiseed"
    d.footer.text = "この便りに心当たりが無いときは事務局へ"

    p = d.add_paragraph("見積書 送付のご案内")
    p.alignment = "center"
    p.runs[0].size_pt = 16
    p.runs[0].bold = True
    p = d.add_paragraph("いつもお世話になっております。次の書類をお送りします。")

    for name, n in [("御見積書", 1), ("カタログ(玄関ドア)", 1), ("返信用封筒", 1)]:
        d.add_paragraph(f"{name} … {n} 部", style="List Bullet")

    p = d.add_paragraph()
    p.add_run("お受け取りの確認欄: ")
    r = p.add_run("                    ")
    r.underline = True
    p.add_run("(サイン)")
    d.save(str(OUT / "送付状.docx"))


# ======================================================================
# 5. 回覧
# ======================================================================
def circular():
    d = Doc()
    a4(d)
    d.add_heading("回覧 — 夏季の節電のお願い", level=1)

    p = d.add_paragraph()
    r = p.add_run("空調は 28℃ 設定")
    r.highlight = "yellow"
    p.add_run(" にご協力ください。")

    p = d.add_paragraph()
    p.add_run("昨年の指針(")
    r = p.add_run("27℃")
    r.strike = True
    p.add_run(" → 28℃)に改めています。")

    p = d.add_paragraph()
    p.add_run("面積あたりの目安は 15W/m")
    r = p.add_run("2")
    r.superscript = True
    p.add_run("、湿度は H")
    r = p.add_run("2")
    r.subscript = True
    p.add_run("O の量で変わります。")

    p = d.add_paragraph("詳しい資料: ")
    p.add_hyperlink("https://github.com/aiseed-dev/officework", "officework の置き場")

    p = d.add_paragraph("確認したら名前の右に日付を書いてください。")
    p.add_comment("今週中にお願いします", author="事務局")

    t = d.add_table(rows=2, cols=5)
    t.style = "Table Grid"
    for j, name in enumerate(["佐藤", "鈴木", "高橋", "田中", "伊藤"]):
        t.cell(0, j).text = name
    d.save(str(OUT / "回覧.docx"))


if __name__ == "__main__":
    for build in (notice, minutes, manual, cover_letter, circular):
        build()
    print("5枚書けた →", OUT)
