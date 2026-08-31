# 注文書.xlsx の手続き「送信」—
# 注文行(品番と数量の入った行)をサーバーへ送る。
#
# 据え付け(1機械1回): 中身を確かめてから
#   ~/.config/officework/plugins/送信.py
# へ写す。以後、注文書を開いて データ > Python のパネルで「@送信」
URL = "http://127.0.0.1:8765/order"

import json
import urllib.request

# **開いている calc に繋ぎます。** `@送信` で呼ぶ手続きは別のプロセスで
# 走るので、`s` や `b` は最初から入っていません(入っているのは
# データ > Python の1行の欄だけです)。同じ置き場の `plugins/天気.py` と
# 同じ形で繋ぎます
from officework import calc as xw


def 送る():
    b = xw.Book.attach()
    s = b.active

    lines = []
    for n in range(7, 17):
        code = s[f"A{n}"].value
        qty = s[f"D{n}"].value
        if code and qty:
            lines.append({"品番": str(code), "数量": int(qty)})
    if not lines:
        print("注文行がありません(品番と数量を入れてから)")
        return
    order = {
        "社名": s["B3"].value or "(未記入)",
        "担当": s["D3"].value or "",
        "明細": lines,
    }
    req = urllib.request.Request(
        URL, json.dumps(order, ensure_ascii=False).encode("utf-8"),
        {"Content-Type": "application/json"})
    r = json.loads(urllib.request.urlopen(req, timeout=5).read().decode("utf-8"))
    print(f"送信しました(受付番号 {r['受付番号']}・明細 {len(lines)} 行)")


送る()
