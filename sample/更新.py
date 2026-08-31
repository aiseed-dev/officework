# 注文書.xlsx の手続き「更新」—
# 品番マスタの写し(H〜J 列)をサーバーの正本と入れ替える。
#
# 据え付け(1機械1回): 中身を確かめてから
#   ~/.config/officework/plugins/更新.py
# へ写す。以後、注文書を開いて データ > Python のパネルで「@更新」
URL = "http://127.0.0.1:8765/catalog.csv"

import csv
import io
import urllib.request

# **開いている calc に繋ぎます。** `@更新` で呼ぶ手続きは別のプロセスで
# 走るので、`s` や `b` は最初から入っていません(入っているのは
# データ > Python の1行の欄だけです)
from officework import calc as xw


def 更新する():
    b = xw.Book.attach()
    s = b.active

    raw = urllib.request.urlopen(URL, timeout=5).read()
    rows = list(csv.reader(io.StringIO(raw.decode("utf-8"))))[1:]
    for i, r in enumerate(rows):
        n = 2 + i
        s[f"H{n}"].value = [[r[0], r[2], int(r[4])]]   # 品番・品名・単価
    for n in range(2 + len(rows), 41):   # 減った分の残骸は消す
        s[f"H{n}:J{n}"].value = [[None, None, None]]
    b.recalc()
    print(f"品番マスタを {len(rows)} 品目に更新しました")


更新する()
