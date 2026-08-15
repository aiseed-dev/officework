# 台帳から Web サイトを組み立てる。中身はすべて架空。
#
#   pip install officework
#   python3 sample/在庫から配り物.py   # 在庫の正本
#   python3 sample/研修の受付.py       # 研修の一覧
#   python3 sample/栽培の記録.py       # 畑の台帳
#   python3 sample/サイトを作る.py
#
# **サイトは台帳の写し**(発注者 2026-08-15「そこから、Web サイトができますね」)。
# 人が直すのは xlsx だけで、**HTML は一度も手で触らない**。
#
# 実在の種苗の会のサイトは、200 品目の在庫が手書きの HTML で保たれていて、
# 在庫の言い方が4通りに割れていた。**手で保つ頁は必ずそうなる。**
#
# **JavaScript は使わない。** 分類の開閉は details、申込は form の POST。
# どちらも JS 以前からある素の仕組みで、電波の細い所でも古い機械でも動く。
#
# 台帳が無ければ**その頁を作らず、無いと言う**(黙って空の頁を置かない)。
import html
import pathlib

from officework import sheet

ここ = pathlib.Path(__file__).resolve().parent
出す先 = ここ / "site"

台帳 = {
    "在庫": ここ / "種の在庫.xlsx",
    "研修": ここ / "研修の一覧.xlsx",
    "栽培": ここ / "栽培の台帳.xlsx",
}

会 = {
    "名前": "たねの畑",
    "住所": "〒000-0000 どこかの県どこかの市1-2-3",
    "電話": "000-000-0000",
    "メール": "tane@example.invalid",
}

CSS = """
body{font-family:sans-serif;max-width:46em;margin:0 auto;padding:1em;line-height:1.8;color:#222}
nav a{margin-right:1em}
h1{border-bottom:3px solid #1b6e3c;padding-bottom:.2em}
table{border-collapse:collapse;width:100%;margin:.6em 0}
th,td{border:1px solid #ccc;padding:.4em .6em;text-align:left}
td.n{text-align:right}
.ari{color:#1b6e3c}.sukoshi{color:#b06000}.nashi{color:#888}.yotei{color:#555}
footer{margin-top:3em;border-top:1px solid #ccc;padding-top:1em;color:#555;font-size:.9em}
details{margin:.6em 0}
"""


def 頁(題, 中身):
    return "\n".join([
        "<!doctype html>", '<html lang="ja"><head><meta charset="utf-8">',
        '<meta name="viewport" content="width=device-width,initial-scale=1">',
        f"<title>{html.escape(題)} — {会['名前']}</title>",
        f"<style>{CSS}</style></head><body>",
        "<nav><a href='index.html'>ホーム</a><a href='tane.html'>種</a>"
        "<a href='hatake.html'>畑の記録</a><a href='kenshu.html'>研修</a>"
        "<a href='moushikomi.html'>会員申込</a></nav>",
        f"<h1>{html.escape(題)}</h1>",
        中身,
        "<footer>",
        f"{html.escape(会['名前'])}<br>{html.escape(会['住所'])}<br>"
        f"電話 {会['電話']} / メール {会['メール']}<br>"
        "この頁は表計算の台帳から作っています。",
        "</footer></body></html>",
    ])


def 読む(名):
    p = 台帳[名]
    if not p.exists():
        return None
    b = sheet.Book.open(p)
    ws = b[b.sheet_names[0]]
    return ws.values()


def 在庫の言い方(数):
    if 数 is None:
        return "販売予定", "yotei"
    if 数 <= 0:
        return "品切れ", "nashi"
    if 数 <= 5:
        return f"残りわずか(あと{数})", "sukoshi"
    return "在庫あり", "ari"


def 種の頁(行):
    分類ごと = {}
    for r in 行[1:]:
        番号, 分類, 品名, 単価, 数, _ = (list(r) + [None] * 6)[:6]
        if 番号:
            分類ごと.setdefault(分類, []).append((番号, 品名, 単価, 数))
    出 = ["<p>その日の在庫です。ご注文はお電話かメールでどうぞ。</p>"]
    for 分類, 品目 in 分類ごと.items():
        あり = sum(1 for _, _, _, 数 in 品目 if 数)
        出.append(f"<details open><summary>{html.escape(分類)}"
                  f"({len(品目)}種・在庫あり {あり})</summary>")
        出.append("<table><tr><th>番号</th><th>品名</th><th>単価</th><th>在庫</th></tr>")
        for 番号, 品名, 単価, 数 in 品目:
            言い方, 印 = 在庫の言い方(数)
            出.append(f"<tr><td>{html.escape(str(番号))}</td>"
                      f"<td>{html.escape(str(品名))}</td>"
                      f'<td class="n">{単価:,}円</td>'
                      f'<td class="{印}">{html.escape(言い方)}</td></tr>')
        出.append("</table></details>")
    return "\n".join(出), sum(len(v) for v in 分類ごと.values())


def 畑の頁(行):
    出 = ["<p>いつ・どの畑で・何を見たか。<b>「何もせず」も記録です</b> — "
          "肥料も薬も入れないので、書けるのは見たことだけ。</p>",
          "<table><tr><th>日</th><th>畑</th><th>品目</th><th>作業</th>"
          "<th>天気</th><th>見たこと</th></tr>"]
    for r in reversed(行[1:]):   # 新しい順
        日, 畑, 品目, 作業, 天気, 見たこと, _ = (list(r) + [None] * 7)[:7]
        if not 日:
            continue
        出.append("<tr>" + "".join(
            f"<td>{html.escape(str(v or ''))}</td>"
            for v in (日, 畑, 品目, 作業, 天気, 見たこと)) + "</tr>")
    出.append("</table>")
    return "\n".join(出), len(行) - 1


def 研修の頁(行):
    出 = ["<p>お申し込みはお電話かメールで。定員になり次第、締め切ります。</p>",
          "<table><tr><th>日</th><th>題</th><th>時間</th><th>場所</th>"
          "<th>受講料</th><th>空き</th></tr>"]
    for r in 行[1:]:
        番号, 題, 日, 時間, 場所, 定員, 残り, 受講料 = (list(r) + [None] * 8)[:8]
        if not 番号:
            continue
        空き = "満席" if not 残り or 残り <= 0 else f"あと{残り}名"
        料 = "無料" if not 受講料 else f"{受講料:,}円"
        出.append("<tr>" + "".join(
            f"<td>{html.escape(str(v))}</td>"
            for v in (日, 題, 時間, 場所, 料, 空き)) + "</tr>")
    出.append("</table>")
    return "\n".join(出), len(行) - 1


def 申込の頁():
    return "\n".join([
        "<p>月に一度、その月にお出しできる種と畑の様子をお送りします。</p>",
        '<form method="post" action="/join">',
        '<p><label>お名前 <input type="text" name="名前" required></label></p>',
        '<p><label>メール <input type="email" name="メール" required></label></p>',
        '<p><label>ご住所 <input type="text" name="住所" size="40"></label></p>',
        '<p><label><input type="checkbox" name="案内可" value="○" required> '
        "月に一度のお便りを受け取ります</label></p>",
        '<p><button type="submit">申し込む</button></p>',
        "</form>",
        "<p><b>いただいた住所とメールは、お便りとお届けにだけ使います。</b>"
        "他へ渡しません。やめたいときはお便りに「不要」と返信してください。</p>",
    ])


if __name__ == "__main__":
    出す先.mkdir(exist_ok=True)
    書いた, 無かった = [], []

    種 = 読む("在庫")
    if 種:
        中身, n = 種の頁(種)
        (出す先 / "tane.html").write_text(頁("種", 中身), encoding="utf-8")
        書いた.append(f"tane.html({n} 品目)")
    else:
        無かった.append("種 — 種の在庫.xlsx がありません(在庫から配り物.py で作る)")

    畑 = 読む("栽培")
    if 畑:
        中身, n = 畑の頁(畑)
        (出す先 / "hatake.html").write_text(頁("畑の記録", 中身), encoding="utf-8")
        書いた.append(f"hatake.html({n} 件)")
    else:
        無かった.append("畑の記録 — 栽培の台帳.xlsx がありません(栽培の記録.py で作る)")

    研 = 読む("研修")
    if 研:
        中身, n = 研修の頁(研)
        (出す先 / "kenshu.html").write_text(頁("研修", 中身), encoding="utf-8")
        書いた.append(f"kenshu.html({n} 件)")
    else:
        無かった.append("研修 — 研修の一覧.xlsx がありません(研修の受付.py で作る)")

    (出す先 / "moushikomi.html").write_text(頁("会員申込", 申込の頁()), encoding="utf-8")
    書いた.append("moushikomi.html")

    案内 = ["<p>種を採り、分け、育て方を持ち寄る会です。</p>", "<ul>"]
    案内 += [f"<li><a href='{n.split('(')[0]}'>{n}</a></li>" for n in 書いた]
    案内.append("</ul>")
    (出す先 / "index.html").write_text(頁("たねの畑", "\n".join(案内)), encoding="utf-8")

    print(f"サイトを作りました: {出す先.name}/")
    for n in 書いた:
        print(f"  {n}")
    for 言い分 in 無かった:
        print(f"  作りませんでした — {言い分}")
    print()
    print("**HTML は手で触らない。** 台帳を直して、もう一度これを走らせる。")
