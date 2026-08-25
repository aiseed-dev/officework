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
    for 段, ラベル, _絵, _obj, 印, ow, _pd, _op in api_taiou.rows():
        # ❌(作らないと決めた物)は手引きも要りません
        if 印 == "❌":
            continue
        p = SAKI / 名(段) / f"{名(ラベル)}.adoc"
        out.append((段, ラベル, 印, ow, p))
    return out


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

(まだ書いていません)

== ダイアログ

(ダイアログが出るときは、選ぶものをここに書きます)

== プログラムから

{python}
"""


def main() -> int:
    r = 一覧()
    無い = [x for x in r if not x[4].exists()]

    if "--make" in sys.argv:
        for 段, ラベル, 印, ow, p in 無い:
            p.parent.mkdir(parents=True, exist_ok=True)
            py = f"[source,python]\n----\n{ow}\n----" if ow else "(まだありません)"
            st = "実装済み" if 印 == "✅" else "未実装"
            p.write_text(下書き.format(ラベル=ラベル, 段=段, python=py,
                                       状態=st, 状態の説明=JOTAI[st]), encoding="utf-8")
        print(f"{len(無い)} 枚の下書きを作りました")
        return 0

    if "--index" in sys.argv:
        o = ["= コマンドの手引き", "", "ボタン1つに1枚です。段ごとに分かれています。", ""]
        いま = None
        for 段, ラベル, 印, ow, p in r:
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
