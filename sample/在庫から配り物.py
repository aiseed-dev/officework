# 在庫の表(xlsx)を正本にして、配り物を作る。中身はすべて架空。
#
#   pip install officework
#   python3 sample/在庫から配り物.py
#
# **なぜこれが要るか**(2026-08-15)。sample には既に「商品マスタの正本は
# サーバー」の見本(catalog_server.py + gen_catalog.py)がある。だが小さな
# 団体にサーバーは無い。**正本は人が calc で直す1枚の表**で、そこから
# 案内も注文書も作るのが実際の姿になる。実在の種苗の団体のサイトを見ると
# 200 品目ぶんの在庫が手書きの HTML で保たれていて、在庫の言い方が
# 「在庫があります!」「在庫がありません。」「ご迷惑をおかけ申し訳ござい
# ません。」「販売予定」の4通りに割れていた。**手で保つ表は必ずこうなる。**
#
# ここで作る物:
#   1. 種の在庫.xlsx   — 正本(人が calc で直す。品名・単価・在庫数)
#   2. 種のカタログ.html — お客が見る頁。**在庫の言い方は在庫数から作る**
#      ので割れようがない。JavaScript は使わない
#   3. 種の注文書.xlsx  — 印刷して FAX で送れる注文用紙(A4 縦・印刷範囲つき)
#
# 表を直して、もう一度これを走らせるだけ。**手で HTML を触らない。**
import html
import pathlib

from officework import sheet

ここ = pathlib.Path(__file__).resolve().parent
正本 = ここ / "種の在庫.xlsx"
頁 = ここ / "種のカタログ.html"
注文書 = ここ / "種の注文書.xlsx"

# 架空の品目。分類・品名・単価・在庫数(0 なら品切れ、None なら販売予定)
タネ = [
    ("野菜", "青しそ", 360, 24), ("野菜", "赤しそ", 360, 0),
    ("野菜", "小松菜", 360, 41), ("野菜", "水菜", 360, 12),
    ("野菜", "春菊", 360, 0), ("野菜", "二十日大根", 360, 33),
    ("野菜", "聖護院かぶ", 420, 7), ("野菜", "打木赤皮甘栗かぼちゃ", 480, 3),
    ("野菜", "鹿ケ谷かぼちゃ", 480, None), ("野菜", "大和真菜", 420, 18),
    ("トマト", "ステラミニトマト", 480, 9), ("トマト", "世界一トマト", 480, 0),
    ("トマト", "ポンデローザ", 520, 5), ("トマト", "黄金トマト", 520, None),
    ("豆", "丹波黒大豆", 400, 26), ("豆", "鞍掛豆", 400, 14),
    ("豆", "花豆", 440, 0), ("豆", "青大豆", 400, 31),
    ("花", "松葉菊", 300, 52), ("花", "千日紅", 300, 8),
    ("花", "綿(和綿)", 380, 0), ("花", "藍", 380, 21),
    ("薬草", "カモミール", 340, 16), ("薬草", "エキナセア", 420, 2),
    ("薬草", "レモンバーム", 340, None), ("薬草", "セントジョーンズワート", 420, 11),
]


def 正本を作る():
    """人が calc で直す表。**この形が正本** — 状態の欄は置かない
    (在庫数から決まる物を人に二重に書かせない)"""
    b = sheet.Book()
    ws = b.active
    ws.title = "在庫"
    ws.append(["番号", "分類", "品名", "単価", "在庫数", "覚え書き"])
    for i, (分類, 品名, 単価, 数) in enumerate(タネ, start=1):
        ws.append([f"{i:04d}", 分類, 品名, 単価, 数, ""])
    # 見出しを太字に(範囲の参照は組の組で返る — openpyxl と同じ書き方)
    for cell in ws["A1:F1"][0]:
        cell.font = sheet.Font(bold=True)
    for col, w in (("A", 8), ("B", 10), ("C", 28), ("D", 10), ("E", 10), ("F", 20)):
        ws.column_dimensions[col].width = w
    for r in range(2, len(タネ) + 2):
        ws.cell(row=r, column=4).number_format = "¥#,##0"
    ws.freeze_panes = "A2"
    b.save(正本)
    return len(タネ)


def 在庫の言い方(数):
    """**在庫数から1つに決める。** 手で書くと必ず割れる所"""
    if 数 is None:
        return "販売予定", "yotei"
    if 数 <= 0:
        return "品切れ", "nashi"
    if 数 <= 5:
        return f"残りわずか(あと{数})", "sukoshi"
    return "在庫あり", "ari"


def 読む():
    b = sheet.Book.open(正本)
    ws = b.active
    行 = []
    for r in ws.values():
        if not r or r[0] == "番号":
            continue
        番号, 分類, 品名, 単価, 数, _ = (list(r) + [None] * 6)[:6]
        行.append((番号, 分類, 品名, 単価, 数))
    return 行


def 頁を作る(行):
    """お客が見る頁。**JavaScript を使わない** — 分類の開閉は details だけ"""
    分類ごと = {}
    for 番号, 分類, 品名, 単価, 数 in 行:
        分類ごと.setdefault(分類, []).append((番号, 品名, 単価, 数))
    出 = [
        "<!doctype html>", '<html lang="ja"><head><meta charset="utf-8">',
        '<meta name="viewport" content="width=device-width,initial-scale=1">',
        "<title>種のカタログ</title><style>",
        "body{font-family:sans-serif;max-width:44em;margin:2em auto;padding:0 1em;line-height:1.7}",
        "table{border-collapse:collapse;width:100%}",
        "th,td{border:1px solid #ccc;padding:.4em .6em;text-align:left}",
        "td.n{text-align:right}",
        ".ari{color:#1b6e3c}.sukoshi{color:#b06000}.nashi{color:#888}.yotei{color:#555}",
        "</style></head><body>",
        "<h1>種のカタログ</h1>",
        "<p>この頁は在庫の表から作っています。注文は注文書に書いて FAX か"
        "メールでお送りください。</p>",
    ]
    for 分類, 品目 in 分類ごと.items():
        在庫あり = sum(1 for _, _, _, 数 in 品目 if 数)
        出.append(f"<details open><summary>{html.escape(分類)}"
                  f"({len(品目)}種・在庫あり {在庫あり})</summary>")
        出.append("<table><tr><th>番号</th><th>品名</th><th>単価</th><th>在庫</th></tr>")
        for 番号, 品名, 単価, 数 in 品目:
            言い方, 印 = 在庫の言い方(数)
            出.append(
                f"<tr><td>{html.escape(str(番号))}</td>"
                f"<td>{html.escape(品名)}</td>"
                f'<td class="n">{単価:,}円</td>'
                f'<td class="{印}">{html.escape(言い方)}</td></tr>'
            )
        出.append("</table></details>")
    出.append("</body></html>")
    頁.write_text("\n".join(出), encoding="utf-8")


def 注文書を作る(行):
    """印刷して FAX で送れる注文用紙。A4 縦・見出し行の繰り返しつき。

    **append を使わず座標で置く。** append は中身が全部空の行を飛ばすので
    (`[""]` でも `[None]` でも進まない)、表題・空行・記入欄・空行・見出し
    という定型の用紙が1行ずつ上にずれる。2026-08-15 にこの見本で見つけた穴。
    用紙は場所が意味を持つので、どのみち座標で置くほうが読みやすい。
    """
    b = sheet.Book()
    ws = b.active
    ws.title = "注文書"

    def 置く(r, c, v, 太字=False, 形式=None):
        cell = ws.cell(row=r, column=c)
        cell.value = v
        if 太字:
            cell.font = sheet.Font(bold=True)
        if 形式:
            cell.number_format = 形式
        return cell

    置く(1, 1, "種の注文書", 太字=True)
    置く(3, 1, "お名前", 太字=True)
    置く(3, 4, "ご連絡先", 太字=True)
    置く(4, 1, "ご住所", 太字=True)
    見出し = 6
    for c, 名 in enumerate(["番号", "品名", "単価", "袋数", "小計", "覚え書き"], start=1):
        置く(見出し, c, 名, 太字=True)

    先頭 = 見出し + 1
    for i, (番号, _, 品名, 単価, 数) in enumerate(行):
        r = 先頭 + i
        # **品番は数で置いて「0000」の形式で見せる。** 文字の "0023" を
        # 置くと数の 23 にされてしまう(上の穴)ので、表計算の作法どおりに
        置く(r, 1, int(番号), 形式="0000")
        置く(r, 2, 品名)
        置く(r, 3, 単価, 形式="¥#,##0")
        置く(r, 5, f"=C{r}*D{r}", 形式="¥#,##0")
        if not 数:
            置く(r, 6, "(品切れ)")
    最終 = 先頭 + len(行) - 1

    合計行 = 最終 + 2
    置く(合計行, 3, "合計", 太字=True)
    置く(合計行, 5, f"=SUM(E{先頭}:E{最終})", 太字=True, 形式="¥#,##0")

    for col, w in (("A", 8), ("B", 28), ("C", 10), ("D", 8), ("E", 12), ("F", 14)):
        ws.column_dimensions[col].width = w
    # 印刷: 見出しの行を2枚目以降にも出す
    ws.print_area = f"A1:F{合計行}"
    ws.print_title_rows = f"{見出し}:{見出し}"
    b.save(注文書)
    return 合計行


if __name__ == "__main__":
    n = 正本を作る()
    print(f"正本を書きました: {正本.name}({n} 品目)")
    行 = 読む()
    頁を作る(行)
    print(f"カタログを書きました: {頁.name}")
    最終 = 注文書を作る(行)
    print(f"注文書を書きました: {注文書.name}(A4 縦・印刷範囲 A1:F{最終})")
    print()
    print("次にすること: 種の在庫.xlsx を calc で開いて在庫数を直し、")
    print("もう一度これを走らせる。HTML は手で触らない。")
