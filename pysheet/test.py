# officework.sheet(pysheet)の検査。Rust 側の tests/python_smoke.rs から呼ばれる。
# 手で回すなら:
#   cargo build -p pysheet
#   mkdir -p /tmp/os/officework && cp target/debug/lib_sheet.so /tmp/os/officework/_sheet.so
#   PYTHONPATH=/tmp/os python3 pysheet/test.py
import os
import sys
import tempfile

from officework import sheet as office_sheet

def check(cond, msg):
    if not cond:
        print(f"NG: {msg}", file=sys.stderr)
        sys.exit(1)

# --- 作って・計算して・保存して・読み直す -----------------------------------
b = office_sheet.Book()
s = b[0]
s["A1"] = "ザボガードF F-02"
s["B1"] = 4
s["C1"] = 125000
s["D1"] = "=B1*C1"
s["D2"] = "=ROUND(D1*0.1,0)"
s["D3"] = "=D1+D2"
check(s["D3"].value == 550000, f"式が計算されない: {s['D3'].value}")
check(s.formula("D3") == "=D1+D2", "編集欄に式が戻らない")
# **整数は int、小数は float**(2026-08-15 に openpyxl に合わせた)。
# 前は何でも float で返していたので、340 が 340.0 になり、品番や個数を
# 見せる前に毎回 int() が要った。中は f64 のまま
check(isinstance(s["B1"].value, int) and s["B1"].value == 4, "整数が int で返らない")
s["B2"] = 4.5
check(isinstance(s["B2"].value, float) and s["B2"].value == 4.5, "小数が float で返らない")
check(s["Z9"].value is None, "空セルが None でない")

with tempfile.TemporaryDirectory() as d:
    out = os.path.join(d, "round.xlsx")
    b.save(out)
    b2 = office_sheet.Book.open(out)
    s2 = b2[0]
    check(s2["A1"].value == "ザボガードF F-02", "日本語が往復しない")
    check(s2["D3"].value == 550000, "式が保存されず再計算できない")
    check(s2.formula("D1") == "=B1*C1", "式そのものが往復しない")

# --- 型: bool は bool のまま、None は消す ------------------------------------
s["E1"] = True
check(s["E1"].value is True, "bool が bool で返らない")
s["A1"] = None
check(s["A1"].value is None, "None で消えない")

# --- 文字列は**そのまま置く**(2026-08-15 に「打ったのと同じ解釈」から改めた)--
#
# 前の決めは「Python から置く字も calc で打った字と同じに読む」で、
# `"123"` は数の 123 に、`"0001"` は数の 1 になっていた。**改めた理由**:
# この package は「openpyxl の代替」と名乗っており、openpyxl は文字を文字の
# まま置く。そして品番・郵便番号・電話番号・会員番号は**頭の 0 が意味を持つ**
# ので、数にされると壊れる(種苗の会の見本で実際に踏んだ)。
# 打鍵の側は変えていない — calc で 0001 と打てば今までどおり数の 1 になる
# (それは Excel と同じで正しい)。**ファイルの口と打鍵の口を分けた**という
# 改めで、経緯は docs/sekkei/python.ja.md
s["F1"] = "123"
check(s["F1"].value == "123", "数字に見える字が数にされた(openpyxl は字のまま)")
s["F2"] = "0001"
check(s["F2"].value == "0001", "品番の頭の 0 が落ちた")
s["F3"] = 123
check(s["F3"].value == 123 and isinstance(s["F3"].value, int), "数を置いたら数で返るべき")

# --- datetime は Excel の通し番号で持ち、読むと datetime で返る ---------------
#
# **中では通し番号、読むと日付**です(openpyxl と同じ)。生の通し番号は
# `ws.nama("G1")` で見られます。2026-08-27 に読む側を本家に合わせました
import datetime
EPOCH = datetime.date(1899, 12, 30)   # Excel の通し番号の起点
d = datetime.date(2026, 8, 5)
s["G1"] = d
check(s.nama("G1") == (d - EPOCH).days, f"date が通し番号にならない: {s.nama('G1')}")
check(s["G1"].value == d, f"日付が date で返らない: {s['G1'].value!r}")
s["G2"] = "=YEAR(G1)"
s["G3"] = "=MONTH(G1)"
check((s["G2"].value, s["G3"].value) == (2026, 8),
      f"通し番号が DATE 関数の規約とずれている: YEAR={s['G2'].value} MONTH={s['G3'].value}")
s["G4"] = datetime.datetime(2026, 8, 5, 18, 0, 0)
check(abs(s.nama("G4") - ((d - EPOCH).days + 0.75)) < 1e-9,
      f"datetime の時刻が日の割合にならない: {s.nama('G4')}")
check(s["G4"].value == datetime.datetime(2026, 8, 5, 18, 0, 0),
      f"日時が datetime で返らない: {s['G4'].value!r}")
s["G5"] = datetime.time(6, 0)
check(abs(s.nama("G5") - 0.25) < 1e-9, f"time が日の割合にならない: {s.nama('G5')}")
check(s["G5"].value == datetime.time(6, 0), f"時刻が time で返らない: {s['G5'].value!r}")
try:
    s["G6"] = object()
    check(False, "置けない型を黙って受けた")
except TypeError:
    pass

# --- 行の出し入れで式の参照も動く(明細行を増やす操作)------------------------
b3 = office_sheet.Book()
t = b3[0]
for i, n in enumerate([100, 200, 300], start=1):
    t[f"A{i}"] = n
t["A4"] = "=SUM(A1:A3)"
t.insert_row(2)                       # 2行目に空行 → 明細が1行増える
check(t.formula("A5") == "=SUM(A1:A4)", f"行を挿しても参照が伸びない: {t.formula('A5')}")
t["A2"] = 50
check(t["A5"].value == 650, f"挿した行に打った値が合計に入らない: {t['A5'].value}")
t.remove_row(2)
check(t["A4"].value == 600, f"行を抜いた後の合計が違う: {t['A4'].value}")

# --- 表示形式は据え置き(値を差し替えても書式が残るのが存在理由)--------------
# 書式は Python からは作れない(作るのは calc の仕事)ので、実物で確かめる
real = "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki/実施要領様式7_提案見積書.xlsx"
if os.path.exists(real):
    rb = office_sheet.Book.open(real)
    rs = rb[0]
    check(rs["B1"].value == "（様式７）", f"実物の中身が読めない: {rs['B1'].value}")
    merges = rs.merges
    check(len(merges) > 0, "実物の様式にセル結合が無いことになっている")
    rows, cols = rs.shape
    check(rows > 0 and cols > 0, "実物の範囲が取れない")
    rs["A30"] = "サンプル商事株式会社"
    rs["C30"] = "=B30*100"
    rs["B30"] = 3
    check(rs["C30"].value == 300, "実物の上で式が効かない")
    with tempfile.TemporaryDirectory() as d:
        out = os.path.join(d, "様式7_差込.xlsx")
        rb.save(out)
        rb2 = office_sheet.Book.open(out)
        rs2 = rb2[0]
        check(rs2["A30"].value == "サンプル商事株式会社", "差し込んだ値が保存されない")
        check(rs2["B1"].value == "（様式７）", "元の内容が壊れた")
        check(rs2.merges == merges, "保存でセル結合が崩れた(帳票の枠が壊れた)")
        import zipfile
        with zipfile.ZipFile(out) as z:
            names = z.namelist()
        check("xl/styles.xml" in names, "書式ごと消えた")
else:
    print("実物の様式が無いので、その分の検査は飛ばした", file=sys.stderr)

# --- polars 連携(polars がある環境でだけ回る)--------------------------------
# 集計は polars、枠は sheet — 分業の橋そのものの検査。
# 開発機では .venv(miniforge)+ abi3 の .so で:
#   cargo build -p pysheet --release --features extension-module
#   mkdir -p /tmp/os && cp target/release/liboffice_sheet.so /tmp/os/office_sheet.so
#   PYTHONPATH=/tmp/os .venv/bin/python pysheet/test.py
try:
    import polars as pl
except ImportError:
    print("polars が無いので、その分の検査は飛ばした", file=sys.stderr)
else:
    df = pl.DataFrame({"品名": ["甲", "甲", "乙"], "金額": [100, 150, 40]})
    g = df.group_by("品名").agg(pl.col("金額").sum()).sort("品名")
    pb = office_sheet.Book()
    ps = pb[0]
    for i, (name, total) in enumerate(g.iter_rows(), start=1):
        ps[f"A{i}"] = name
        ps[f"B{i}"] = float(total)
    ps["B3"] = "=SUM(B1:B2)"
    check(ps["B3"].value == 290, f"polars の集計が差し込めない: {ps['B3'].value}")
    back = pl.DataFrame(ps.values(), orient="row")
    check(back.shape == (3, 2), f"values() が DataFrame にならない: {back.shape}")
    # docs/ja/df-manual.adoc「Python で書くと」の例。=df(売上[金額] = 売上[単価] * 売上[数量])
    # と =df(税率 = 0.1, 売上[税額] = 売上[金額] * 税率) を polars で書いた物
    売上 = pl.DataFrame({
        "品名": ["A4 コピー用紙", "トナー", "ファイル"],
        "単価": [420, 8900, 180],
        "数量": [30, 2, 50],
    })
    売上 = 売上.with_columns((pl.col("単価") * pl.col("数量")).alias("金額"))
    税率 = 0.1
    売上 = 売上.with_columns((pl.col("金額") * 税率).alias("税額"))
    check(売上["金額"].to_list() == [12600, 17800, 9000], f"df の手引きの金額が違う: {売上['金額'].to_list()}")
    check([round(x) for x in 売上["税額"].to_list()] == [1260, 1780, 900], f"df の手引きの税額が違う: {売上['税額'].to_list()}")

# ── シートの名前は文字列でも渡せる(remove / copy_worksheet)────────────
# 前は Sheet しか受けず、名前を渡すと `.title` が無くて TypeError で
# 止まっていました。openpyxl の見本は Sheet を渡しますが、手引きは名前で
# 書いている所があります
b = office_sheet.Book()
b.create_sheet("控え")
c = b.copy_worksheet("控え")
check(c.title == "控え Copy", f"名前で写した物の名前が違う: {c.title}")
b.remove("控え Copy")
check(b.sheetnames == ["Sheet1", "控え"], f"名前で抜けない: {b.sheetnames}")
b.remove(b["控え"])
check(b.sheetnames == ["Sheet1"], f"Sheet で抜けない: {b.sheetnames}")

# ── 入力規則の文言(入力メッセージ・エラーメッセージ)が xlsx に残る ──────
# 前は add_data_validation が promptTitle / prompt / errorTitle / error を
# 渡していなかったので、Excel で開いても文言が出ませんでした
with tempfile.TemporaryDirectory() as t:
    b = office_sheet.Book()
    s = b[0]
    dv = office_sheet.DataValidation(type="list", formula1='"見積,注文"', allow_blank=True)
    dv.promptTitle = "区分"
    dv.prompt = "一覧から選びます"
    dv.errorTitle = "区分が違います"
    dv.error = "見積か注文にしてください"
    dv.add("B2:B10")
    s.add_data_validation(dv)
    out = os.path.join(t, "規則.xlsx")
    b.save(out)
    import zipfile
    with zipfile.ZipFile(out) as z:
        x = z.read("xl/worksheets/sheet1.xml").decode("utf-8")
    for k in ('promptTitle="区分"', 'prompt="一覧から選びます"',
              'errorTitle="区分が違います"', 'error="見積か注文にしてください"',
              'errorStyle="stop"'):
        check(k in x, f"入力規則の文言が xlsx に無い: {k}")
    m = office_sheet.load_workbook(out)[0]._s.validation_messages
    check(m == [("B2:B10", "区分", "一覧から選びます", "stop", "区分が違います", "見積か注文にしてください")],
          f"読み直した入力規則の文言が違う: {m}")

print("OK")
