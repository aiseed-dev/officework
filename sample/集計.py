# 売上台帳.xlsx を読んで区分ごとに集計する — 読み側の見本(アプリ不要)。
#
#   pip install officework
#   python3 集計.py
#
# values() は「行×欄の2次元リスト」を1回で返す(1セルずつ引くより速い)。
# 込み入った集計はここから polars / pandas に渡せばよい —
# エンジンの仕事は「帳票を正しく読む」まで。
from officework import sheet

b = sheet.Book.open("売上台帳.xlsx")
rows = b[b.sheet_names[0]].values()
head, body = rows[0], rows[1:]
print("見出し:", head, f"/ 明細 {len(body)} 行")

goukei: dict[str, float] = {}
for r in body:
    kubun, kingaku = r[1], r[5]
    goukei[kubun] = goukei.get(kubun, 0) + (kingaku or 0)

for k, v in sorted(goukei.items(), key=lambda x: -x[1]):
    print(f"{k}\t{v:>10,.0f} 円")
print(f"総計\t{sum(goukei.values()):>10,.0f} 円")
