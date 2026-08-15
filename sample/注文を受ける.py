# 返ってきた注文書を受けて、台帳に載せ、在庫を引き、納品書を作る。中身は架空。
#
#   pip install officework
#   python3 sample/在庫から配り物.py     # 先に正本と注文書を作る
#   python3 sample/注文を受ける.py
#
# 「表計算が正本」の一周の後半(前半は 在庫から配り物.py)。
# 注文は FAX かメールで返ってくる — **サーバーは無い**。返ってきた注文書を
# 開いて、台帳に足し、在庫を引き、納品書と宛名を作るまで。
#
# **断ることを黙ってしない。** 在庫より多い注文は受けず、どの行をなぜ
# 受けなかったかを言う(受けた分だけ台帳に載る)。ここを黙って通すと、
# 台帳と在庫と現物が別々のことを言い出す。
import datetime
import pathlib

from officework import doc, sheet

ここ = pathlib.Path(__file__).resolve().parent
正本 = ここ / "種の在庫.xlsx"
注文書 = ここ / "種の注文書_記入済み.xlsx"
台帳 = ここ / "種の受注台帳.xlsx"
納品書 = ここ / "種の納品書.docx"

# **入金の突き合わせは当面は手でやる**(発注者 2026-08-15)。通帳の CSV と
# 突き合わせる仕掛けは作らない。**ただし書く場所は要る** — 欄が無ければ
# 手でもできない。「まだ」を人が「済」に直し、日を入れる
台帳の見出し = ["受付日", "お名前", "番号", "品名", "袋数", "単価", "金額",
                "入金", "入金日", "発送"]


def 記入済みの注文書を作る():
    """**返ってきた注文書の代わり。** 実物では FAX かメールで届く。
    見本を単体で走らせられるように、ここで書き込んだ物を作る"""
    b = sheet.Book.open(ここ / "種の注文書.xlsx")
    ws = b[b.sheet_names[0]]
    ws["B3"] = "山田 花子"
    ws["E3"] = "090-0000-0000"
    ws["B4"] = "〒000-0000 どこかの県どこかの市1-2-3"
    # 袋数の欄(D)に書き込む。**断る3つの理由が全部出る取り合わせ**にする
    # (行 = 6 + 品目の番号。0001 が 7 行目)
    #   0001 青しそ  在庫24 → 2袋: 通る
    #   0007 聖護院かぶ 在庫7 → 3袋: 通る
    #   0002 赤しそ  在庫0  → 1袋: 品切れで断る
    #   0009 鹿ケ谷かぼちゃ 在庫なし → 2袋: まだ販売していないので断る
    #   0024 エキナセア 在庫2 → 5袋: 足りないので断る
    for 品目, 袋 in ((1, 2), (7, 3), (2, 1), (9, 2), (24, 5)):
        ws[f"D{6 + 品目}"] = 袋
    b.save(注文書)


def 注文を読む():
    b = sheet.Book.open(注文書)
    ws = b[b.sheet_names[0]]
    宛先 = (ws["B3"], ws["E3"], ws["B4"])
    注文 = []
    for r in range(7, ws.max_row + 1):
        袋 = ws[f"D{r}"]
        if not 袋:
            continue
        注文.append((ws[f"A{r}"], ws[f"B{r}"], ws[f"C{r}"], int(袋)))
    return 宛先, 注文


def 在庫を引く(注文):
    """受けられる分だけ引いて保存する。**足りない行は受けない。**
    返り: (受けた行, 断った行の言い分)"""
    b = sheet.Book.open(正本)
    ws = b[b.sheet_names[0]]
    # 番号 → その行
    どこ = {}
    for r in range(2, ws.max_row + 1):
        番号 = ws[f"A{r}"]
        if 番号 is not None:
            どこ[str(番号)] = r
    受けた, 断った = [], []
    for 番号, 品名, 単価, 袋 in 注文:
        鍵 = f"{int(番号):04d}" if isinstance(番号, int) else str(番号)
        r = どこ.get(鍵)
        if r is None:
            断った.append(f"{鍵} {品名}: この番号は在庫の表にありません")
            continue
        今 = ws[f"E{r}"]
        if 今 is None:
            断った.append(f"{鍵} {品名}: まだ販売していません(販売予定)")
            continue
        if 今 < 袋:
            断った.append(f"{鍵} {品名}: {袋}袋のご注文ですが在庫は{今}袋です")
            continue
        ws[f"E{r}"] = 今 - 袋
        受けた.append((鍵, 品名, 単価, 袋))
    b.save(正本)
    return 受けた, 断った


def 台帳に載せる(受付日, 名前, 受けた):
    """**足すだけ。** 台帳は消さない — 過去の受注は動かさないのが台帳"""
    if 台帳.exists():
        b = sheet.Book.open(台帳)
        ws = b[b.sheet_names[0]]
    else:
        b = sheet.Book()
        ws = b.active
        ws.title = "受注台帳"
        for c, 名 in enumerate(台帳の見出し, start=1):
            cell = ws.cell(row=1, column=c)
            cell.value = 名
            cell.font = sheet.Font(bold=True)
        for col, w in (("A", 12), ("B", 16), ("C", 8), ("D", 26), ("E", 8),
                       ("F", 10), ("G", 12), ("H", 8), ("I", 12), ("J", 8)):
            ws.column_dimensions[col].width = w
        ws.freeze_panes = "A2"
    for 番号, 品名, 単価, 袋 in 受けた:
        r = ws.max_row + 1
        ws.cell(row=r, column=1).value = 受付日
        ws.cell(row=r, column=2).value = 名前
        ws.cell(row=r, column=3).value = 番号
        ws.cell(row=r, column=4).value = 品名
        ws.cell(row=r, column=5).value = 袋
        c6 = ws.cell(row=r, column=6)
        c6.value = 単価
        c6.number_format = "¥#,##0"
        c7 = ws.cell(row=r, column=7)
        c7.value = f"=E{r}*F{r}"
        c7.number_format = "¥#,##0"
        ws.cell(row=r, column=8).value = "まだ"   # 入金(手で「済」に直す)
        ws.cell(row=r, column=10).value = "まだ"  # 発送
    b.save(台帳)
    return ws.max_row - 1


def 納品書を作る(受付日, 宛先, 受けた, 断った):
    """**差し込みは Python がやる。** 型紙に流し込むのと同じ仕事を、
    docx を組み立てて出す(writer の差し込みの機能はまだ無い)"""
    名前, 連絡先, 住所 = 宛先
    d = doc.Doc()
    d.add_heading("納 品 書", level=1)
    d.add_paragraph(f"{住所}")
    d.add_paragraph(f"{名前} 様")
    d.add_paragraph("")
    d.add_paragraph(f"受付日: {受付日}   ご連絡先: {連絡先}")
    d.add_paragraph("このたびはご注文ありがとうございます。下記のとおりお送りします。")

    t = d.add_table(rows=1, cols=4)
    for i, 名 in enumerate(["番号", "品名", "袋数", "金額"]):
        t.rows[0].cells[i].text = 名
    合計 = 0
    for 番号, 品名, 単価, 袋 in 受けた:
        金額 = 単価 * 袋
        合計 += 金額
        row = t.add_row()
        for i, v in enumerate([番号, 品名, str(袋), f"{金額:,}円"]):
            row.cells[i].text = str(v)
    row = t.add_row()
    row.cells[2].text = "合計"
    row.cells[3].text = f"{合計:,}円"

    if 断った:
        d.add_paragraph("")
        d.add_paragraph("お受けできなかった品(申し訳ありません)")
        for 言い分 in 断った:
            d.add_paragraph(f"・{言い分}")
    d.save(納品書)
    return 合計


if __name__ == "__main__":
    記入済みの注文書を作る()
    print(f"返ってきた注文書(見本): {注文書.name}")
    宛先, 注文 = 注文を読む()
    print(f"注文: {len(注文)} 行 / お名前: {宛先[0]}")

    受けた, 断った = 在庫を引く(注文)
    受付日 = datetime.date(2026, 8, 15).isoformat()
    件数 = 台帳に載せる(受付日, 宛先[0], 受けた)
    合計 = 納品書を作る(受付日, 宛先, 受けた, 断った)

    print(f"受けた: {len(受けた)} 行(合計 {合計:,}円)")
    for 言い分 in 断った:
        print(f"  受けられません — {言い分}")
    print(f"台帳: {台帳.name}({件数} 行)")
    print(f"納品書: {納品書.name}")
    print()
    print("在庫は引いてあります。カタログを作り直すには:")
    print("  python3 sample/在庫から配り物.py")
