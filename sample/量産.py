# 見積書.xlsx を型紙にして、宛先ごとの見積書を量産する(アプリ不要)。
#
#   pip install officework
#   python3 量産.py
#
# openpyxl の定番の使い方(帳票の差し込み)を、書式を壊さずにやる。
# 毎回 open() から始めるので、前の宛先の値が次に混ざらない。
from officework import sheet

atesaki = [
    ("株式会社みほん商事 御中", 120),
    ("有限会社れいじ建設 御中", 200),
    ("見本町自治会 御中", 80),
]

for i, (name, m2) in enumerate(atesaki, 1):
    b = sheet.Book.open("見積書.xlsx")     # 型紙から毎回
    s = b[b.sheet_names[0]]
    s["A3"] = name                          # 宛名
    s["C12"] = m2                           # 足場の㎡
    s["C13"] = m2                           # 高圧洗浄も同じ広さ
    out = f"見積書_{i:02}.xlsx"
    b.save(out)
    total = sheet.Book.open(out)            # 式は読み直しで計算済み
    f18 = total[total.sheet_names[0]]["F18"].value
    print(f"{out}  {name}  合計 {f18:,.0f} 円")
