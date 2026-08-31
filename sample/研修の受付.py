# 研修(講座・交換会)の受付。中身はすべて架空。
#
#   pip install officework
#   python3 sample/研修の受付.py
#
# **物を売るのと同じ道具で、場も開ける**(発注者 2026-08-15「研修の開催でも
# 同じものがつかえるでしょう」)。実際に同じ型で書けるかを確かめた台本。
#
#   種の在庫  → 研修の一覧(**定員が在庫**)
#   受注台帳  → 参加者の名簿
#   在庫を引く → 席を引く(満席なら断って理由を言う)
#   注文書     → 受付の名簿(当日、紙で持つ)
#   月の便り   → 開催の案内(同じ客の台帳から)
#
# **違うのは3つだけ**だった: 日時と場所がある / 当日の出欠がある /
# 満席のときキャンセル待ちに回す。それ以外は物を売るのと同じ。
import pathlib

from officework import sheet

ここ = pathlib.Path(__file__).resolve().parent
研修の一覧 = ここ / "研修の一覧.xlsx"
名簿 = ここ / "研修の名簿.xlsx"
受付表 = ここ / "研修の受付表.xlsx"

見出し_研修 = ["番号", "題", "日", "時間", "場所", "定員", "残り", "受講料"]
見出し_名簿 = ["受付日", "研修", "お名前", "連絡先", "受講料", "入金", "出欠", "覚え書き"]

見本の研修 = [
    ("001", "自家採種のはじめかた", "2026-09-12", "13:30〜16:00",
     "公民館 第2会議室", 20, 20, 2000),
    # 畑の見学は**定員が小さい**(2名)。満席で断る道を見本に通すため
    ("002", "無肥料の畑の見学会", "2026-09-27", "10:00〜12:00",
     "たねの畑", 2, 2, 1000),
    ("003", "種の交換会(秋)", "2026-10-18", "13:00〜16:00",
     "公民館 大ホール", 60, 60, 0),
]

# 申込(実物では申込の頁の POST か、電話・はがきを書き取る)
見本の申込 = [
    ("001", "山田 花子", "hanako@example.invalid"),
    ("001", "鈴木 一郎", "090-0000-0000"),
    ("002", "佐藤 みどり", "midori@example.invalid"),
    ("002", "高橋 太郎", "080-1111-1111"),
    ("002", "中村 さくら", "sakura@example.invalid"),   # ここで満席
    ("999", "田中 実", "minoru@example.invalid"),        # 無い研修
]


def 研修の一覧を作る():
    if 研修の一覧.exists():
        return 0
    b = sheet.Book()
    ws = b.active
    ws.title = "研修"
    for c, 名 in enumerate(見出し_研修, start=1):
        cell = ws.cell(row=1, column=c)
        cell.value = 名
        cell.font = sheet.Font(bold=True)
    for i, 行 in enumerate(見本の研修, start=2):
        for c, v in enumerate(行, start=1):
            ws.cell(row=i, column=c).value = v
        ws.cell(row=i, column=8).number_format = "¥#,##0"
    for col, w in (("A", 8), ("B", 26), ("C", 12), ("D", 14),
                   ("E", 20), ("F", 8), ("G", 8), ("H", 10)):
        ws.column_dimensions[col].width = w
    ws.freeze_panes = "A2"
    b.save(研修の一覧)
    return len(見本の研修)


def 席を引く(申込):
    """**在庫を引くのと同じ。** 満席なら受けず、理由を言う。
    受けた分だけ名簿に載る(台帳と現物がずれない)"""
    b = sheet.Book.open(研修の一覧)
    ws = b[b.sheet_names[0]]
    どこ = {}
    for r in range(2, ws.max_row + 1):
        番号 = ws[f"A{r}"].value
        if 番号 is not None:
            どこ[str(番号)] = r
    受けた, 断った = [], []
    for 研修番号, 名前, 連絡先 in 申込:
        r = どこ.get(str(研修番号))
        if r is None:
            断った.append(f"{名前}: 研修 {研修番号} は一覧にありません")
            continue
        題 = ws[f"B{r}"].value
        残り = ws[f"G{r}"].value
        受講料 = ws[f"H{r}"].value
        if not 残り or 残り <= 0:
            断った.append(f"{名前}: 「{題}」は満席です(キャンセル待ちへ)")
            continue
        ws[f"G{r}"] = 残り - 1
        受けた.append((研修番号, 題, 名前, 連絡先, 受講料))
    b.save(研修の一覧)
    return 受けた, 断った


def 名簿に載せる(受付日, 受けた):
    """**受注台帳と同じ。** 足すだけ — 過去の申込は動かさない"""
    if 名簿.exists():
        b = sheet.Book.open(名簿)
        ws = b[b.sheet_names[0]]
    else:
        b = sheet.Book()
        ws = b.active
        ws.title = "名簿"
        for c, 名 in enumerate(見出し_名簿, start=1):
            cell = ws.cell(row=1, column=c)
            cell.value = 名
            cell.font = sheet.Font(bold=True)
        for col, w in (("A", 12), ("B", 26), ("C", 16), ("D", 24),
                       ("E", 10), ("F", 8), ("G", 8), ("H", 24)):
            ws.column_dimensions[col].width = w
        ws.freeze_panes = "A2"
    for _, 題, 名前, 連絡先, 受講料 in 受けた:
        r = ws.max_row + 1
        for c, v in enumerate([受付日, 題, 名前, 連絡先, 受講料, "まだ", "", ""], start=1):
            ws.cell(row=r, column=c).value = v
        ws.cell(row=r, column=5).number_format = "¥#,##0"
    b.save(名簿)
    return ws.max_row - 1


def 受付表を作る(研修番号):
    """**当日、紙で持つ名簿。** 印刷範囲つき A4。出欠の欄は手で丸を付ける —
    会場で電池も電波も要らないのが紙の強み"""
    b0 = sheet.Book.open(研修の一覧)
    w0 = b0[b0.sheet_names[0]]
    情報 = None
    for r in range(2, w0.max_row + 1):
        if str(w0[f"A{r}"].value) == str(研修番号):
            情報 = tuple(w0[f"{c}{r}"].value for c in "BCDEFG")
    if 情報 is None:
        return None
    題, 日, 時間, 場所, 定員, 残り = 情報

    b1 = sheet.Book.open(名簿)
    w1 = b1[b1.sheet_names[0]]
    参加 = [r for r in w1.values()[1:] if r and r[1] == 題]

    b = sheet.Book()
    ws = b.active
    ws.title = "受付"

    def 置く(r, c, v, 太字=False):
        cell = ws.cell(row=r, column=c)
        cell.value = v
        if 太字:
            cell.font = sheet.Font(bold=True)

    置く(1, 1, f"受付表 — {題}", 太字=True)
    置く(2, 1, f"{日} {時間} / {場所}")
    置く(3, 1, f"定員 {定員} 名 / 申込 {定員 - 残り} 名")
    見出し = 5
    for c, 名 in enumerate(["出欠", "お名前", "連絡先", "受講料", "入金", "覚え書き"], start=1):
        置く(見出し, c, 名, 太字=True)
    for i, r0 in enumerate(参加):
        r = 見出し + 1 + i
        置く(r, 1, "□")
        置く(r, 2, r0[2])
        置く(r, 3, r0[3])
        c = ws.cell(row=r, column=4)
        c.value = r0[4]
        c.number_format = "¥#,##0"
        置く(r, 5, r0[5])
    最終 = 見出し + len(参加)
    for col, w in (("A", 6), ("B", 18), ("C", 24), ("D", 10), ("E", 8), ("F", 24)):
        ws.column_dimensions[col].width = w
    ws.print_area = f"A1:F{最終}"
    ws.print_title_rows = f"{見出し}:{見出し}"
    b.save(受付表)
    return len(参加)


if __name__ == "__main__":
    n = 研修の一覧を作る()
    if n:
        print(f"研修の一覧: {研修の一覧.name}({n} 件)")

    受けた, 断った = 席を引く(見本の申込)
    件数 = 名簿に載せる("2026-08-15", 受けた)
    print(f"受けた申込: {len(受けた)} 件 / 名簿 {件数} 行")
    for 言い分 in 断った:
        print(f"  受けられません — {言い分}")

    人数 = 受付表を作る("001")
    print(f"受付表: {受付表.name}(001 の参加 {人数} 名・A4 印刷範囲つき)")
    print()
    print("案内は 月の便り.py と同じ客の台帳から出せます(同意の縛りも同じ)。")
