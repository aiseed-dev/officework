# 見積書.xlsx に値を差し込む — エンジンだけの見本(アプリ不要)。
#
#   pip install officework
#   python3 sample/差し込み.py
#
# openpyxl と違い、罫線・結合・列幅・表示形式(¥#,##0)・印刷範囲を
# 保ったまま値だけ差し替わる。式は読み直すと計算済みの値になっている。
import pathlib

from officework import sheet

# 開くファイルは、この .py の隣から探します(どこで走らせても同じ)
ここ = pathlib.Path(__file__).resolve().parent

b = sheet.Book.open(ここ / "見積書.xlsx")
print("シート:", b.sheet_names)
print("読めなかった部品:", b.unsupported)  # 黙って落とさず、ここに出る

s = b[b.sheet_names[0]]
s["A3"] = "株式会社みほん工業 御中"   # 宛名(書式は据え置き)
s["C12"] = 150                        # 足場 120㎡ → 150㎡
b.save(ここ / "見積書_差し込み.xlsx")

# 読み直すと式(=C12*E12、小計・消費税・合計)は計算済みの値で見える
a = sheet.Book.open(ここ / "見積書_差し込み.xlsx")
t = a[a.sheet_names[0]]
print("金額 F12 =", t["F12"].value)   # 150 × 800 = 120000
print("合計 F18 =", t["F18"].value)   # 666600(消費税まで追従)
