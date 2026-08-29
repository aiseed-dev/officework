"""図の試験 — データが図形になって、紙にも xlsx にも出るか。

    .venv/bin/python pysheet/test_chart.py

図は指図ではなく**図形の集まり**です(2026-08-27 発注者「チャートは
python による独自描画でいいのでは」)。だから見るのは「図形が正しい
場所に正しい大きさで置かれたか」です。
"""
import os
import sys
import tempfile
import zipfile

from officework import chart
from officework import sheet as office_sheet

warui = 0


def check(cond, msg):
    global warui
    if not cond:
        print("NG:", msg, file=sys.stderr)
        warui += 1


def hyou():
    b = office_sheet.Book()
    ws = b[0]
    ws["A3"] = "支店"
    ws["B3"] = "目標"
    ws["C3"] = "実績"
    for i, (m, mo, ji) in enumerate(
        [("札幌", 120, 140), ("仙台", 180, 150), ("東京", 240, 280)]
    ):
        ws.cell(4 + i, 1).value = m
        ws.cell(4 + i, 2).value = mo
        ws.cell(4 + i, 3).value = ji
    return b, ws


# ── スケール(d3 の芯)──────────────────────────────────────────

lin = chart.Linear([0, 100], [200, 0])
check(lin(0) == 200, "linear: 下端が合わない")
check(lin(100) == 0, "linear: 上端が合わない")
check(lin(50) == 100, "linear: 真ん中が合わない")
check(lin.ticks(5) == [0, 20, 40, 60, 80, 100], f"目盛りが読みにくい: {lin.ticks(5)}")

# 端数のある範囲でも、目盛りは 1 / 2 / 5 の倍数に寄る
me = chart.Linear([0, 273], [0, 100]).ticks(5)
check(all(abs(v % 50) < 1e-9 for v in me), f"目盛りが 50 刻みでない: {me}")

band = chart.Band(["あ", "い", "う"], [0, 300], padding=0.0)
check(band("あ") == 0.0, "band: 1つ目の左端")
check(abs(band.width - 100.0) < 1e-9, f"band: 幅が違う {band.width}")
check(abs(band.center("い") - 150.0) < 1e-9, f"band: 真ん中が違う {band.center('い')}")

# 隙間を空けると、棒は細くなるが**位置の間隔は変わらない**
sukima = chart.Band(["あ", "い"], [0, 200], padding=0.5)
check(abs(sukima.width - 50.0) < 1e-9, f"隙間つきの幅: {sukima.width}")
check(abs(sukima("い") - sukima("あ") - 100.0) < 1e-9, "隙間で間隔まで変わった")

# ── 近道が図形を置く ────────────────────────────────────────────

b, ws = hyou()
mae = len(ws.shapes)
ws.add_chart("bar", data="B3:C6", categories="A4:A6", at="A10", title="目標と実績")
bou = len(ws.shapes) - mae
# 枠 + 題 + 軸2本 + 目盛りの線と字 + 区分の字3つ + 棒6本
check(bou > 15, f"棒グラフの図形が少なすぎる: {bou}")
check(any(s.get("fill") == "4472C4" for s in ws.shapes), "1系列目の色が付いていない")
check(any(s.get("fill") == "ED7D31" for s in ws.shapes), "2系列目の色が付いていない")
check(any(s.get("text") == "目標と実績" for s in ws.shapes), "題が入っていない")
check(any(s.get("text") == "札幌" for s in ws.shapes), "区分の名前が入っていない")

# **系列は列ごと**。1行目が字なら見出しとして外す
atai = ws._hani_no_atai("B3:C6")
check(atai == [[120.0, 180.0, 240.0], [140.0, 150.0, 280.0]],
      f"範囲の読み取りが違う: {atai}")

# 円グラフは扇を自由な形で置く
b2, ws2 = hyou()
ws2.add_chart("pie", data="C4:C6", categories="A4:A6", at="A10", title="内訳")
ougi = [s for s in ws2.shapes if s["kind"] == "path"]
check(len(ougi) == 3, f"扇が3つ出ていない: {len(ougi)}")
check(all(len(s["points"]) >= 4 for s in ougi), "扇が多角形に刻まれていない")

# ドーナツは中を抜く(外周と内周で点が倍になる)
b3, ws3 = hyou()
ws3.add_chart("doughnut", data="C4:C6", categories="A4:A6", at="A10")
naka = [s for s in ws3.shapes if s["kind"] == "path"]
check(len(naka[0]["points"]) > len(ougi[0]["points"]),
      "ドーナツの内周が無い")

# 折れ線は**塗らない**(塗ると閉じて余計な線が出る)
b4, ws4 = hyou()
ws4.add_chart("line", data="C3:C6", categories="A4:A6", at="A10")
ori = [s for s in ws4.shapes if s["kind"] == "path"]
check(len(ori) == 1, f"折れ線が1本でない: {len(ori)}")
check(ori[0].get("fill") is None, "折れ線に塗りが付いている(閉じてしまう)")

# 知らない種類は断る
# (前はここが "radar" でした。レーダーチャートを作れるようになったので、
#  試験のほうが古くなって落ちていました。2026-08-30)
try:
    ws.add_chart("そんな図はない", data="C4:C6", at="A30")
    check(False, "知らない図の種類が黙って通った")
except ValueError:
    pass

# ── 置いた場所 ────────────────────────────────────────────────

b5, ws5 = hyou()
ws5.add_chart("bar", data="C4:C6", categories="A4:A6", at="D20",
              width=200, height=120)
waku = [s for s in ws5.shapes if s["kind"] == "rect" and s["width"] == 200.0]
check(waku, "枠が置かれていない")
check(waku[0]["at"] == "D20", f"留めるセルが違う: {waku[0]['at']}")
# 中身は枠と同じセルに留めて、ずらしで置く(セルの粗さに縛られない)
check(all(s["at"] == "D20" for s in ws5.shapes), "図形の留め先がばらけている")

# ── xlsx と PDF に出る ──────────────────────────────────────────

tmp = tempfile.mkdtemp(prefix="chart-")
x = os.path.join(tmp, "図.xlsx")
b.save(x)
z = zipfile.ZipFile(x)
zu = [n for n in z.namelist() if n.startswith("xl/drawings/") and n.endswith(".xml")]
check(zu, "xlsx に図の部品が無い")
xml = z.read(zu[0]).decode("utf-8")
check(xml.count("<xdr:sp") >= bou, "xlsx に図形が出ていない")
check("4472C4" in xml, "xlsx に色が出ていない")
check("目標と実績" in xml, "xlsx に題が出ていない")

p = os.path.join(tmp, "図.pdf")
b.save(p)
data = open(p, "rb").read()
check(data.startswith(b"%PDF"), "PDF になっていない")
check(len(data) > 3000, f"PDF が小さすぎる(図が出ていない?): {len(data)}")

print("OK" if warui == 0 else f"{warui} 件おかしい")
sys.exit(1 if warui else 0)
