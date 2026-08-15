# 栽培の記録(畑の台帳)。中身はすべて架空。
#
#   pip install officework
#   python3 sample/栽培の記録.py
#
# **これも同じ形**(発注者 2026-08-15「栽培の記録も同じでしょう」)。
# 在庫・受注・名簿と同じ「1行が1件の台帳」で、足すだけ・消さない。
#
# **自然栽培では「何もしなかった」も記録**になる。肥料も薬も入れないのだから、
# 書くのは「見たこと」— いつ・どの畑で・何が・どうだったか。翌年の材料は
# そこにしかない。作業の欄に「(何もせず)」と書ける形にしてある。
#
# 写真の台帳とは**ファイル名で結ぶ**。台帳の中に画像を持たない —
# 写真は写真の台帳の持ち物で、こちらはその名前を指すだけ。
import pathlib

from officework import sheet

ここ = pathlib.Path(__file__).resolve().parent
台帳 = ここ / "栽培の台帳.xlsx"

見出し = ["日", "畑", "品目", "作業", "天気", "見たこと", "写真"]

見本 = [
    ("2026-04-02", "南の畑", "青しそ", "播種", "晴", "去年のこぼれ種も出ている", ""),
    ("2026-04-20", "南の畑", "青しそ", "(何もせず)", "曇", "草に負けていない。間引かない", ""),
    ("2026-05-06", "東の畑", "聖護院かぶ", "播種", "雨のち曇", "土が湿りすぎ。少し遅らせた", ""),
    ("2026-06-04", "南の畑", "青しそ", "(何もせず)", "晴", "本葉4枚。虫食いは数枚だけ", "001_青しそ.jpg"),
    ("2026-06-07", "東の畑", "聖護院かぶ", "間引き", "晴", "小ぶり。雨が少ない年の形", "002_聖護院かぶ.jpg"),
    ("2026-06-10", "西の畑", "丹波黒大豆", "土寄せ", "曇", "莢がふくらみ始めた", "003_丹波黒大豆.jpg"),
    ("2026-07-15", "南の畑", "青しそ", "採種のため残す", "晴", "穂が立った株を10本残す", ""),
    ("2026-08-01", "西の畑", "藍", "刈り取り", "晴", "一番刈り。乾きが早い", "005_藍.jpg"),
]


def 台帳を作る():
    if 台帳.exists():
        return 0
    b = sheet.Book()
    ws = b.active
    ws.title = "栽培"
    for c, 名 in enumerate(見出し, start=1):
        cell = ws.cell(row=1, column=c)
        cell.value = 名
        cell.font = sheet.Font(bold=True)
    for i, 行 in enumerate(見本, start=2):
        for c, v in enumerate(行, start=1):
            ws.cell(row=i, column=c).value = v
    for col, w in (("A", 12), ("B", 12), ("C", 16), ("D", 18),
                   ("E", 12), ("F", 40), ("G", 22)):
        ws.column_dimensions[col].width = w
    ws.freeze_panes = "A2"
    b.save(台帳)
    return len(見本)


def 読む():
    if not 台帳.exists():
        return []
    b = sheet.Book.open(台帳)
    ws = b[b.sheet_names[0]]
    out = []
    for r in ws.values()[1:]:
        日, 畑, 品目, 作業, 天気, 見たこと, 写真 = (list(r) + [None] * 7)[:7]
        if 日:
            out.append((str(日), 畑, 品目, 作業, 天気, 見たこと, 写真))
    return out


if __name__ == "__main__":
    n = 台帳を作る()
    if n:
        print(f"栽培の台帳を作りました: {台帳.name}({n} 件)")
    行 = 読む()
    畑ごと = {}
    for _, 畑, *_ in 行:
        畑ごと[畑] = 畑ごと.get(畑, 0) + 1
    print(f"記録: {len(行)} 件 / " + " ".join(f"{k} {v}件" for k, v in 畑ごと.items()))
    なにもせず = sum(1 for r in 行 if r[3] and "何もせず" in r[3])
    print(f"  うち「何もせず」{なにもせず} 件 — **これも記録**")
    print()
    print("サイトに出すには: python3 sample/サイトを作る.py")
