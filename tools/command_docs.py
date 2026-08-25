#!/usr/bin/env python3
"""コマンドごとの手引きの入れ物を作り、抜けを見る。

    python3 tools/command_docs.py           # 何が書けていないかを出す
    python3 tools/command_docs.py --make    # 足りないファイルの下書きを作る
    python3 tools/command_docs.py --index   # 目次を書き直す

*置き場は段ごとのフォルダ*です。

....
docs/commands/
├── README.ja.adoc      目次(この道具が起こす)
├── ファイル/
│   ├── 開く.adoc
│   └── 保存.adoc
├── ホーム/
│   ├── 太字.adoc
│   └── …
└── …
....

**1つのボタンに1枚**です。書くのは3つ — 何をするか、ダイアログで何を選ぶか、
プログラムからどう書くか。ダイアログの中の選択肢は対応表に入らないので、
*ここが唯一の置き場*になります。

抜けは対応表(`tools/api_taiou.py`)の行から数えます。
表に載っているのに手引きが無いボタンは、この道具が名指しします。
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).parent))
import api_taiou  # noqa: E402

SAKI = ROOT / "docs/commands"

# ファイル名に使えない字を置き換える
NG = re.compile(r'[/\\:*?"<>|]')


def 名(ラベル: str) -> str:
    return NG.sub("_", ラベル).strip() or "無題"


def 一覧():
    """(段, ラベル, 印, officework, ファイルの場所)"""
    out = []
    for 段, ラベル, _絵, _obj, 印, ow, pd, op in api_taiou.rows():
        # ❌(作らないと決めた物)は手引きも要りません
        if 印 == "❌":
            continue
        p = SAKI / 名(段) / f"{名(ラベル)}.adoc"
        out.append((段, ラベル, 印, ow, p, pd, op))
    return out


# 状態。**手引きは実物より先に書きます**(2026-08-24 発注者)。
# 画面の灰色のボタンと同じ考え方で、*あることは見せて、状態で言う*。
JOTAI = {
    "実装済み": "いま使えます",
    "未実装": "*まだ使えません。* これから作ります",
    "廃止予定": "*なくす予定です。* 行き先を下に書いてあります",
}

# ボタンの名前 → キー(`face/src/keys.rs` と `keys_doc.py` の説明から結ぶ)
def _キーの表():
    import keys_doc
    import face_dummy  # noqa: F401  (使いません)
    return {}


# 種類ごとの、正直な一言。**その通りにしか動かない物**だけ書きます
KUBUN = [
    (("太字", "斜体", "下線", "取り消し線", "上付き", "下付き"),
     "選んだ字に掛けます。もう一度押すと外れます。"),
    (("左揃え", "中央揃え", "右揃え", "両端揃え", "均等割付"),
     "段落の揃え方を変えます。選んでいる段落に掛かります。"),
    (("フォント", "フォントのサイズ", "フォントの色", "塗りつぶしの色"),
     "選んだ所の見た目を変えます。"),
    (("フォントサイズの拡大", "フォントサイズの縮小"),
     "選んだ字の大きさを1段ずつ変えます。"),
    (("コピー", "切り取り", "貼り付け"),
     "選んだものを写す・移す・入れます。字でも、セルでも、画像でも同じです。"),
    (("元に戻す", "やり直し"), "直前の1手を戻す・やり直します。"),
    (("昇順並べ替え", "降順並べ替え"), "選んだ範囲を並べ替えます。"),
    (("数値の書式", "通貨スタイル", "パーセントのスタイル", "カンマスタイル"),
     "セルの見せ方を変えます。*中の値は変わりません*。"),
]


def 一言(ラベル: str) -> str:
    for 名前, 文 in KUBUN:
        if ラベル in 名前:
            return 文
    return "(まだ書いていません)"


# 状態。**手引きは実物より先に書きます**(2026-08-24 発注者)。
# 画面の灰色のボタンと同じ考え方で、*あることは見せて、状態で言う*。
JOTAI = {
    "実装済み": "いま使えます",
    "未実装": "*まだ使えません。* これから作ります",
    "廃止予定": "*なくす予定です。* 行き先を下に書いてあります",
}

下書き = """= {ラベル}

{段}にあります。

*状態: {状態}* — {状態の説明}

== 何をするか

{一言}

== ダイアログ

{ダイアログ}

== プログラムから

{python}
{本家}
"""


def main() -> int:
    r = 一覧()
    無い = [x for x in r if not x[4].exists()]

    if "--make" in sys.argv:
        for 段, ラベル, 印, ow, p, pd, op in 無い:
            p.parent.mkdir(parents=True, exist_ok=True)
            if 印 == "✍":
                書き = ow or api_taiou.理由(ラベル, "✍") or ""
                py = ("専用の呼び方はありません。こう書けば同じことができます。\n\n"
                      f"[source,python]\n----\n{書き}\n----")
            elif ow:
                py = f"[source,python]\n----\n{ow}\n----"
            else:
                py = "(まだありません)"
            本家 = ""
            もと = [x for x in ((pd, "python-docx"), (op, "openpyxl"))
                    if x[0] and x[0] != "—"]
            if もと:
                本家 = "\n他のライブラリでは、こう書きます。\n\n[cols=\"1,2\"]\n|===\n|道具 |書き方\n\n"
                本家 += "\n".join(f"|{名} |`{書}`" for 書, 名 in もと) + "\n|===\n"
            st = {"✅": "実装済み", "✍": "実装済み"}.get(印, "未実装")
            p.write_text(下書き.format(ラベル=ラベル, 段=段, python=py, 本家=本家,
                                       状態=st, 状態の説明=JOTAI[st],
                                       一言=一言(ラベル),
                                       ダイアログ="(ダイアログは出ません)" if 印 == "✅"
                                       else "(ダイアログが出るときは、選ぶものをここに書きます)"),
                        encoding="utf-8")
        print(f"{len(無い)} 枚の下書きを作りました")
        return 0

    if "--index" in sys.argv:
        o = ["= コマンドの手引き", "", "ボタン1つに1枚です。段ごとに分かれています。", ""]
        いま = None
        for 段, ラベル, 印, ow, p, *_ in r:
            if 段 != いま:
                o.append(f"== {段}")
                o.append("")
                いま = 段
            if p.exists():
                o.append(f"* link:{名(段)}/{名(ラベル)}.adoc[{ラベル}]")
            else:
                o.append(f"* {ラベル}(まだ)")
        SAKI.mkdir(parents=True, exist_ok=True)
        (SAKI / "README.ja.adoc").write_text("\n".join(o) + "\n", encoding="utf-8")
        print(f"目次を書きました({len(r)} 項目)")
        return 0

    # **状態ごとに数えます。** 手引きは実物より先に書くので、
    # 「未実装」の枚数がこれから作る物の一覧になります
    数 = {}
    for q in sorted(SAKI.rglob("*.adoc")):
        if q.name == "README.ja.adoc":
            continue
        m = re.search(r"\*状態: (実装済み|未実装|廃止予定)\*", q.read_text(encoding="utf-8"))
        数[m.group(1) if m else "印が無い"] = 数.get(m.group(1) if m else "印が無い", 0) + 1
    print(f"手引きが要るボタン {len(r)} 枚のうち、書けているのは {len(r) - len(無い)} 枚です")
    print("書いた手引きの状態:")
    for k in ("実装済み", "未実装", "廃止予定", "印が無い"):
        if 数.get(k):
            print(f"  {k:<8} {数[k]} 枚")
    if 無い:
        print("\nまだ無いもの(段ごとの数):")
        から = {}
        for 段, *_ in 無い:
            から[段] = から.get(段, 0) + 1
        for 段, n in から.items():
            print(f"  {段:<24} {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
