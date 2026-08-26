#!/usr/bin/env python3
"""officework エンジンで、test/write_xlsx_openpyxl.py と同じ5枚を作る。

    .venv/bin/python test/write_xlsx_officework.py [出力先フォルダ(既定 test/out/officework)]

出来上がりの名前は同じ(見積書.xlsx など)。openpyxl の側と並べて目で
見比べます。エンジンに口が無い機能は飛ばして、終わりに一覧で出ます —
**その一覧が「できない事の台帳」**です。黙って欠けません。
"""
import sys
import datetime as dt
from contextlib import contextmanager
from pathlib import Path

from officework import sheet
from officework.sheet import (Font, PatternFill, Border, Side, Alignment,
                              Protection, Comment, DataValidation, Table,
                              TableStyleInfo)

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent / "out" / "officework"
OUT.mkdir(parents=True, exist_ok=True)

SKIPPED = []


@contextmanager
def attempt(label):
    """口が無い・形が違うときは飛ばして控える(黙って欠けない)。"""
    try:
        yield
    except Exception as e:
        SKIPPED.append(f"{label}: {type(e).__name__}: {e}")


THIN = Side(style="thin")
MEDIUM = Side(style="medium")
BOX = Border(top=THIN, bottom=THIN, left=THIN, right=THIN)
UNDER = Border(bottom=THIN)


def a4(ws, landscape=False):
    with attempt("用紙 A4 と向き"):
        ws.paper_size = ws.PAPERSIZE_A4
        if landscape:
            ws.orientation = ws.ORIENTATION_LANDSCAPE
    with attempt("1ページに収める"):
        ws.fit_to_page = True


# ======================================================================
# 1. 見積書
# ======================================================================
def quote():
    wb = sheet.Book()
    ws = wb[0]
    ws.title = "見積書"
    for col, w in zip("ABCDEF", (4, 22, 8, 6, 12, 14)):
        with attempt(f"列幅 {col}"):
            ws.column_dimensions[col].width = w

    ws.merge_cells("A1:F1")
    c = ws.cell(1, 1)
    c.value = "御 見 積 書"
    c.font = Font(size=18, bold=True)
    c.alignment = Alignment(horizontal="center")

    c = ws.cell(2, 5)
    c.value = dt.date(2026, 8, 26)
    c.number_format = "yyyy年m月d日"
    ws.cell(3, 5).value = "見積番号"
    c = ws.cell(3, 6)
    c.value = "Q-2026-0826"
    c.number_format = "@"

    ws.merge_cells("A5:C5")
    c = ws.cell(5, 1)
    c.value = "日本フネン株式会社 御中"
    c.font = Font(size=12, bold=True, underline="single")

    c = ws.cell(5, 5)
    c.value = "aiseed"
    with attempt("リンク(外の URL)"):
        c.hyperlink = "https://github.com/aiseed-dev/officework"
    c.font = Font(color="0563C1", underline="single")
    ws.cell(6, 5).value = "担当: 営業部"

    with attempt("名前つきスタイルの定義"):
        wb.add_named_style("表の見出し",
                           font=Font(bold=True, color="FFFFFF"),
                           fill=PatternFill("solid", fgColor="1B6E3C"),
                           alignment=Alignment(horizontal="center"))
    c = ws.cell(8, 6)
    c.value = 0.1
    c.number_format = "0%"
    with attempt("名前の定義(税率)"):
        wb.create_named_range("税率", "見積書!$F$8")
    ws.cell(8, 5).value = "消費税率"

    for j, label in enumerate(["No.", "品名", "数量", "単位", "単価", "金額"], start=1):
        c = ws.cell(10, j)
        c.value = label
        with attempt("名前つきスタイルを当てる"):
            c.style = "表の見出し"
        c.border = BOX
    items = [("玄関ドア 親子 断熱", 1, "組", 458000),
             ("採光サイドパネル", 1, "枚", 76000),
             ("電気錠セット", 1, "式", 92000)]
    for i, (name, qty, unit, price) in enumerate(items, start=11):
        c = ws.cell(i, 1)
        c.value = i - 10
        c.alignment = Alignment(horizontal="center")
        ws.cell(i, 2).value = name
        c = ws.cell(i, 3)
        c.value = qty
        c.alignment = Alignment(horizontal="right")
        c = ws.cell(i, 4)
        c.value = unit
        c.alignment = Alignment(horizontal="center")
        c = ws.cell(i, 5)
        c.value = price
        c.number_format = "#,##0"
        c = ws.cell(i, 6)
        c.value = f"=C{i}*E{i}"
        c.number_format = "#,##0"
        for j in range(1, 7):
            ws.cell(i, j).border = BOX
    ws.cell(15, 5).value = "小計"
    ws.cell(15, 6).value = "=SUM(F11:F13)"
    ws.cell(16, 5).value = "消費税"
    with attempt("式の中の名前(=F15*税率)"):
        ws.cell(16, 6).value = "=F15*税率"
    c = ws.cell(17, 5)
    c.value = "合計"
    c.font = Font(bold=True)
    c = ws.cell(17, 6)
    c.value = "=F15+F16"
    c.font = Font(bold=True)
    for r in (15, 16, 17):
        ws.cell(r, 6).number_format = '"¥"#,##0'
        ws.cell(r, 5).border = UNDER
        ws.cell(r, 6).border = UNDER

    ws.merge_cells("A19:F19")
    ws.cell(19, 1).value = "備考: 記入欄(黄色)だけ書き替えられます。ほかは保護しています。"
    ws.cell(20, 2).value = "納期のご希望"
    memo = ws.cell(20, 3)
    memo.fill = PatternFill("solid", fgColor="FFF2CC")
    with attempt("セルのロックを外す"):
        memo.protection = Protection(locked=False)
    memo.border = BOX
    with attempt("シートの保護"):
        ws.protect()

    a4(ws)
    with attempt("フッター"):
        ws.oddFooter.center.text = "この見積の有効期限は発行から30日です"
    with attempt("ブックの情報(題)"):
        wb.properties.title = "御見積書"
    wb.save(str(OUT / "見積書.xlsx"))


# ======================================================================
# 2. 月次売上報告
# ======================================================================
def monthly_report():
    wb = sheet.Book()
    ws = wb[0]
    ws.title = "8月"
    with attempt("枠線を消す"):
        ws.show_gridlines = False
    with attempt("固定(A4)"):
        ws.freeze_panes = "A4"
    for col, w in zip("ABCDE", (12, 10, 10, 10, 10)):
        with attempt(f"列幅 {col}"):
            ws.column_dimensions[col].width = w

    ws.merge_cells("A1:E1")
    c = ws.cell(1, 1)
    c.value = "月次売上報告(2026年8月)"
    c.font = Font(size=14, bold=True)

    logo = Path(__file__).resolve().parent.parent / "packaging/icons/hicolor/128x128/officework.png"
    if logo.exists():
        with attempt("画像"):
            ws.add_image(str(logo), "E2")

    header = ["支店", "目標", "実績", "達成率", "前月比"]
    data = [["本店", 5000, 5420], ["駅前店", 3000, 2710],
            ["西店", 2000, 2150], ["北店", 1500, 1180]]
    for j, h in enumerate(header, start=1):
        ws.cell(3, j).value = h
    for i, (name, plan, act) in enumerate(data, start=4):
        ws.cell(i, 1).value = name
        c = ws.cell(i, 2)
        c.value = plan
        c.number_format = "#,##0"
        c = ws.cell(i, 3)
        c.value = act
        c.number_format = "#,##0"
        c = ws.cell(i, 4)
        c.value = f"=C{i}/B{i}"
        c.number_format = "0.0%"
        c = ws.cell(i, 5)
        c.value = (0.06, -0.04, 0.11, -0.12)[i - 4]
        c.number_format = "+0.0%;-0.0%"
    with attempt("テーブル(スタイルと縞々)"):
        t = Table(displayName="Sales", ref="A3:E7")
        t.tableStyleInfo = TableStyleInfo(name="TableStyleMedium9", showRowStripes=True)
        ws.add_table(t)

    with attempt("条件付き書式(データバー)"):
        ws.conditional_formatting_databar("C4:C7", color="638EC6")
    with attempt("条件付き書式(色の濃淡)"):
        ws.conditional_formatting_colorscale("D4:D7")
    with attempt("条件付き書式(0未満を赤字)"):
        ws.conditional_formatting_cellis("E4:E7", "lessThan", "0", font=Font(color="9C0006"))

    with attempt("グラフ(縦棒+折れ線)"):
        ws.add_chart("bar", data="B3:C7", categories="A4:A7", at="A9", title="目標と実績")
    with attempt("グラフ(円)"):
        ws.add_chart("pie", data="C3:C7", categories="A4:A7", at="F9", title="実績の内訳")

    a4(ws, landscape=True)
    with attempt("ブックの情報(題)"):
        wb.properties.title = "月次売上報告"
    wb.save(str(OUT / "月次売上報告.xlsx"))


# ======================================================================
# 3. 出勤簿
# ======================================================================
def attendance():
    wb = sheet.Book()
    ws = wb[0]
    ws.title = "出勤簿"
    for col, w in zip("ABCDEF", (12, 8, 10, 10, 10, 16)):
        with attempt(f"列幅 {col}"):
            ws.column_dimensions[col].width = w

    ws.merge_cells("A1:F1")
    c = ws.cell(1, 1)
    c.value = "出勤簿(2026年8月・第4週)"
    c.font = Font(size=13, bold=True)

    for j, h in enumerate(["日付", "区分", "出勤", "退勤", "実働", "備考"], start=1):
        c = ws.cell(3, j)
        c.value = h
        c.font = Font(bold=True)
        c.fill = PatternFill("solid", fgColor="DDEBF7")
        c.border = BOX

    with attempt("入力規則(リスト+入力時と誤りの文言)"):
        kinds = DataValidation(type="list", formula1='"出勤,休暇,半休,出張"', allow_blank=True)
        kinds.promptTitle = "区分"
        kinds.prompt = "一覧から選びます"
        kinds.errorTitle = "区分が違います"
        kinds.error = "出勤・休暇・半休・出張のどれかにしてください"
        ws.add_data_validation(kinds)
        kinds.add("B4:B10")
    with attempt("入力規則(時刻の範囲)"):
        hours = DataValidation(type="time", operator="between",
                               formula1="TIME(6,0,0)", formula2="TIME(23,0,0)")
        ws.add_data_validation(hours)
        hours.add("C4:D10")

    base = dt.date(2026, 8, 24)
    for i in range(7):
        r = 4 + i
        c = ws.cell(r, 1)
        c.value = base + dt.timedelta(days=i)
        c.number_format = "m月d日(aaa)"
        ws.cell(r, 2).value = "出勤" if i < 5 else "休暇"
        if i < 5:
            s = ws.cell(r, 3)
            s.value = dt.time(9, 0)
            e = ws.cell(r, 4)
            e.value = dt.time(17, 45 if i != 2 else 30)
            s.number_format = e.number_format = "h:mm"
            w = ws.cell(r, 5)
            w.value = f"=D{r}-C{r}-TIME(1,0,0)"
            w.number_format = "[h]:mm"
        for j in range(1, 7):
            ws.cell(r, j).border = BOX
    c = ws.cell(6, 6)
    c.value = "客先で採寸"
    with attempt("コメント(作成者つき)"):
        c.comment = Comment("日本フネンの現場です", "上長")

    with attempt("条件付き書式(数式の条件で土日に色)"):
        ws.conditional_formatting_formula("A4:F10", "WEEKDAY($A4,2)>=6",
                                          fill=PatternFill("solid", fgColor="FCE4EC"))

    ws.cell(12, 4).value = "実働の合計"
    t = ws.cell(12, 5)
    t.value = "=SUM(E4:E10)"
    t.number_format = "[h]:mm"
    t.font = Font(bold=True)

    with attempt("タイトル行の印刷"):
        ws.print_title_rows = "1:3"
    with attempt("改ページ(12行目の下)"):
        ws.add_row_break(12)
    a4(ws)
    wb.save(str(OUT / "出勤簿.xlsx"))


# ======================================================================
# 4. 棚卸表
# ======================================================================
def inventory():
    wb = sheet.Book()
    ws = wb[0]
    ws.title = "棚卸"
    with attempt("シート見出しの色"):
        ws.tab_color = "C00000"
    for col, w in zip("ABCDE", (10, 22, 8, 10, 12)):
        with attempt(f"列幅 {col}"):
            ws.column_dimensions[col].width = w

    prev = wb.create_sheet("前月")
    prev.cell(1, 1).value = "品番"
    prev.cell(1, 2).value = "数"
    for i, (code, n) in enumerate([("00123", 40), ("00456", 12), ("00789", 5)], start=2):
        c = prev.cell(i, 1)
        c.value = code
        c.number_format = "@"
        prev.cell(i, 2).value = n
    with attempt("シートを隠す(hidden)"):
        prev.sheet_state = "hidden"
    conf = wb.create_sheet("設定")
    conf.cell(1, 1).value = "棚卸の基準日"
    conf.cell(1, 2).value = dt.date(2026, 8, 31)
    with attempt("シートを深く隠す(veryHidden)"):
        conf.sheet_state = "veryHidden"

    ws.merge_cells("A1:E1")
    c = ws.cell(1, 1)
    c.value = "棚卸表(倉庫A)"
    c.font = Font(size=13, bold=True)

    for j, h in enumerate(["品番", "品名", "実数", "前月", "差"], start=1):
        c = ws.cell(3, j)
        c.value = h
        c.font = Font(bold=True)
        c.border = BOX
    rows = [("00123", "玄関ドア用 丁番", 38), ("00456", "サイドパネル ガラス", 12),
            ("00789", "電気錠 制御基板", 4), ("00123", "玄関ドア用 丁番(再掲の誤り)", 2)]
    for i, (code, name, n) in enumerate(rows, start=4):
        c = ws.cell(i, 1)
        c.value = code
        c.number_format = "@"          # 頭の 0 を守る
        ws.cell(i, 2).value = name
        ws.cell(i, 3).value = n
        ws.cell(i, 4).value = f'=IFERROR(VLOOKUP(A{i},前月!A:B,2,FALSE),0)'
        ws.cell(i, 5).value = f"=C{i}-D{i}"
        for j in range(1, 6):
            ws.cell(i, j).border = BOX
    with attempt("条件付き書式(重複の色)"):
        ws.conditional_formatting_duplicates("A4:A7",
                                             fill=PatternFill("solid", fgColor="FFC7CE"))
    with attempt("オートフィルター"):
        ws.auto_filter = "A3:E7"
    with attempt("行のグループ化"):
        ws.group_rows(5, 6)
    ws.cell(9, 3).value = "実数の計"
    with attempt("配列式"):
        ws.cell(9, 4).array_formula = "=SUM(C4:C7*1)"
    ws.cell(11, 1).value = "基準日"
    c = ws.cell(11, 2)
    c.value = "=設定!B1"
    c.number_format = "yyyy/m/d"
    c = ws.cell(12, 1)
    c.value = "前月の一覧へ"
    with attempt("リンク(ブックの中へ)"):
        c.hyperlink = "#前月!A1"
    c.font = Font(color="0563C1", underline="single")

    a4(ws)
    wb.save(str(OUT / "棚卸表.xlsx"))


# ======================================================================
# 5. 申込書
# ======================================================================
def application_form():
    wb = sheet.Book()
    ws = wb[0]
    ws.title = "申込書"
    for col, w in zip("ABCDEFGH", (3, 4, 14, 14, 14, 14, 6, 3)):
        with attempt(f"列幅 {col}"):
            ws.column_dimensions[col].width = w

    ws.merge_cells("B2:G2")
    t = ws.cell(2, 2)
    t.value = "会 員 申 込 書"
    t.font = Font(size=16, bold=True, color="FFFFFF")
    with attempt("グラデーションの塗り"):
        t.fill = sheet.GradientFill(stop=("1B6E3C", "63BE7B"))
    t.alignment = Alignment(horizontal="center", vertical="center")
    with attempt("行の高さ"):
        ws.row_dimensions[2].height = 28

    ws.merge_cells("B3:G3")
    c = ws.cell(3, 2)
    with attempt("リッチテキスト(1セルの中で書式替え)"):
        c.rich_text = [("太枠の中だけ ", None), ("黒のボールペン", Font(bold=True)),
                       (" でご記入ください(", None), ("必須", Font(color="FF0000")),
                       (" は空欄にできません)", None)]
    if not c.value:
        c.value = "太枠の中だけ 黒のボールペン でご記入ください(必須 は空欄にできません)"
    c.font = Font(size=9, color="808080")

    ws.merge_cells("B5:B12")
    side = ws.cell(5, 2)
    side.value = "申込者"
    with attempt("縦書き(回転 255)"):
        side.alignment = Alignment(horizontal="center", vertical="center", text_rotation=255)
    side.border = Border(left=MEDIUM, top=MEDIUM, bottom=MEDIUM, right=THIN)
    side.fill = PatternFill("solid", fgColor="EFEFEF")

    def field(row, label, span=("D", "F"), required=False):
        c = ws.cell(row, 3)
        c.value = label + ("(必須)" if required else "")
        c.font = Font(size=10, bold=required, color="C00000" if required else "000000")
        with attempt("字下げ"):
            c.alignment = Alignment(indent=1)
        ws.merge_cells(f"{span[0]}{row}:{span[1]}{row}")
        c = ws.cell(row, 4)
        with attempt("セルのロックを外す"):
            c.protection = Protection(locked=False)
        c.border = BOX
        return c

    field(5, "ふりがな")
    field(6, "お名前", required=True)
    field(7, "生年月日", required=True).number_format = "yyyy年m月d日"
    field(8, "電話番号", required=True).number_format = "@"
    field(9, "メール")
    addr = field(10, "ご住所", required=True)
    addr.alignment = Alignment(wrap_text=True)
    with attempt("行の高さ"):
        ws.row_dimensions[10].height = 30
    with attempt("縮小して全体を表示"):
        field(11, "ご紹介者").alignment = Alignment(shrink_to_fit=True)
    plan = field(12, "コース", span=("D", "D"), required=True)
    with attempt("入力規則(リスト)"):
        dv = DataValidation(type="list", formula1='"A(月2回),B(月4回),家族"')
        ws.add_data_validation(dv)
        dv.add("D12")

    for r in range(5, 13):
        ws.cell(r, 7).border = Border(right=MEDIUM)
    for col in range(2, 8):
        ws.cell(4, col).border = Border(bottom=MEDIUM)
        ws.cell(13, col).border = Border(top=MEDIUM)

    ws.merge_cells("C15:F15")
    c = ws.cell(15, 3)
    c.value = "受付印"
    c.alignment = Alignment(horizontal="right", vertical="top")
    ws.merge_cells("F16:F18")
    stamp = ws.cell(16, 6)
    with attempt("対角線の罫線"):
        stamp.border = Border(top=THIN, bottom=THIN, left=THIN, right=THIN,
                              diagonal=Side(style="hair"), diagonalDown=True)
    ws.cell(16, 3).value = "斜線の枠は事務局が使います"
    with attempt("シートの保護"):
        ws.protect()
    a4(ws)
    with attempt("ヘッダー"):
        ws.oddHeader.right.text = "様式1"
    wb.save(str(OUT / "申込書.xlsx"))


if __name__ == "__main__":
    for build in (quote, monthly_report, attendance, inventory, application_form):
        build()
    print("5枚書けた →", OUT)
    if SKIPPED:
        print(f"\nこの機械ではできなかった事({len(SKIPPED)} 件):")
        for s in SKIPPED:
            print("  ×", s)
    else:
        print("全部の口が通った")
