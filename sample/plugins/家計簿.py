"""レシートの写真を読んで家計簿に足す — officework の見本(手続き)

**置き場**: このファイルを `~/.config/office/plugins/` にコピーすると、
calc から呼べるようになります(calc を開き直してください)。

    @家計簿              写真を選んで、読んで、表に足す
    @家計簿.月まとめ     月ごとの合計を出す

## 読み取りは AI にさせます

レシートは感熱紙で薄く、斜めに撮れ、店ごとに並びが違います。**手元の
OCR では金額を読み違えます** — 家計簿で金額を間違えるのは、読めないより
悪い。だから写真をそのまま AI に渡し、「店・日付・品目・金額」を JSON で
返させます。

**AI が使えなければ、この見本は動きません。名指しで断ります。**
できない物をできるように見せない、がこのソフトの決めです。

用意する物:

- `ANTHROPIC_API_KEY` を環境変数に置く(officework 本体の AI と同じ鍵)

**鍵での認証だけを使います。** 手元の実行体の認証に相乗りする道は
2026-08-15 にやめました — Anthropic の規約が、第三者の製品で
claude.ai のログインと枠を提供することを(事前の承認なしには)
許していないためです。

## なぜ手続き(@家計簿)なのか

セルの関数(=読む(...))にはしません。関数は網に出ない決めですし、
写真を読むのは「押した時に一度だけ」の仕事だからです。表に置いた後は
ただの値なので、開き直しても勝手に読み直しません。
"""

import base64
import json
import mimetypes
import os
import urllib.request

from officework import calc as xw

# AI への言いつけ。**返す形を先に決めておく** — 自由に書かせると、
# 「合計が読めませんでした」のような散文が返って表に入らない
言いつけ = """あなたはレシートの写真から数字を読み取る道具です。
渡された画像から次を読み、**JSON だけ**を返してください(前置き・説明・
コードフェンスは書かない)。

{"店": "店名", "日付": "YYYY-MM-DD", "合計": 数,
 "品目": [{"品": "名前", "金額": 数}, ...],
 "分類": "食費|日用品|交通|医療|交際|その他"}

- 金額は数だけ(円記号・カンマは付けない)
- 読めない項目は null にする。**推測で埋めない**
- 日付が和暦なら西暦に直す
- 分類は品目から素直に決める(迷ったら その他)"""


def _画像を渡せる形に(path):
    """画像を base64 と種類にする(API に渡す形)"""
    kind = mimetypes.guess_type(path)[0] or "image/jpeg"
    with open(path, "rb") as f:
        return base64.b64encode(f.read()).decode(), kind


def _api_で読む(path):
    """Anthropic の API で読む(鍵は環境変数からだけ — 本体と同じ決め)"""
    key = os.environ.get("ANTHROPIC_API_KEY")
    if not key:
        return None
    b64, kind = _画像を渡せる形に(path)
    body = json.dumps({
        "model": os.environ.get("JO_AI_MODEL", "claude-sonnet-5"),
        "max_tokens": 2048,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "image",
                 "source": {"type": "base64", "media_type": kind, "data": b64}},
                {"type": "text", "text": 言いつけ},
            ],
        }],
    }).encode()
    req = urllib.request.Request(
        "https://api.anthropic.com/v1/messages",
        data=body,
        headers={
            "x-api-key": key,
            "anthropic-version": "2023-06-01",
            "content-type": "application/json",
        },
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        d = json.load(r)
    return "".join(c.get("text", "") for c in d.get("content", []))


def 読む(path):
    """写真1枚を読んで、辞書で返す。読めなければ理由を言って止まる"""
    if not os.path.exists(path):
        raise SystemExit(f"写真がありません: {path}")

    答え = _api_で読む(path)
    if 答え is None:
        raise SystemExit(
            "AI が使えないので読めません。"
            "ANTHROPIC_API_KEY を環境変数に置いてください"
        )
    # JSON だけを取り出す(前置きが付いてきた時のため)
    s = 答え[答え.find("{"): 答え.rfind("}") + 1]
    try:
        return json.loads(s)
    except json.JSONDecodeError:
        raise SystemExit(f"AI の答えが読めません: {答え[:120]}")


def 足す(*写真):
    """写真を読んで、表の続きに足す。

    引数を渡さなければ、いま選んでいるセルに書いてある径路を読みます
    (A1 に写真の径路を並べておいて、選んで @家計簿 と打つ使い方)。
    """
    b = xw.Book.attach()
    s = b.active

    paths = list(写真)
    if not paths:
        v = b.selection.value
        if isinstance(v, list):
            paths = [str(x) for row in v for x in (row if isinstance(row, list) else [row])
                     if x]
        elif v:
            paths = [str(v)]
    if not paths:
        raise SystemExit(
            "写真の径路を渡すか、径路を書いたセルを選んでください"
            "(例: @家計簿.足す ~/写真/レシート.jpg)"
        )

    # 見出しが無ければ置く(初めて使うとき)
    if not s["A1"]:
        s["A1"] = [["日付", "店", "分類", "金額", "品目"]]

    # 続きの行を探す(表の下端の次)
    行 = 2
    while s[f"A{行}"]:
        行 += 1

    足した = 0
    for p in paths:
        p = os.path.expanduser(str(p))
        r = 読む(p)
        品 = "・".join(x.get("品", "") for x in (r.get("品目") or [])[:5])
        s[f"A{行}"] = [[
            r.get("日付"), r.get("店"), r.get("分類"), r.get("合計"), 品,
        ]]
        行 += 1
        足した += 1

    print(f"{足した} 枚を読んで {行 - 足した}〜{行 - 1} 行目に足しました")


def 月まとめ():
    """日付の月ごとに、分類別の合計を出す(表の右側に置く)"""
    b = xw.Book.attach()
    s = b.active

    行, 明細 = 2, []
    while True:
        日 = s[f"A{行}"]
        if not 日:
            break
        明細.append((str(日)[:7], s[f"C{行}"] or "その他", float(s[f"D{行}"] or 0)))
        行 += 1
    if not 明細:
        raise SystemExit("集計する明細がありません(@家計簿 で先に足してください)")

    月 = sorted({m for m, _, _ in 明細})
    分類 = sorted({c for _, c, _ in 明細})
    表 = [["月"] + 分類 + ["合計"]]
    for m in 月:
        行値 = [sum(a for mm, cc, a in 明細 if mm == m and cc == c) for c in 分類]
        表.append([m] + 行値 + [sum(行値)])

    s["G1"] = 表
    print(f"{len(月)} か月ぶんを G1 からまとめました")


def main():
    足す()
