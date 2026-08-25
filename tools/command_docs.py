#!/usr/bin/env python3
"""コマンドごとの手引きの入れ物を作り、抜けを見る。

    python3 tools/command_docs.py           # 何が書けていないかを出す
    python3 tools/command_docs.py --make    # 足りないファイルの下書きを作る
    python3 tools/command_docs.py --index   # 目次を書き直す

*置き場は段ごとのフォルダ*です。

....
docs/ja/commands/
├── README.adoc      目次(この道具が起こす)
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

SAKI = ROOT / "docs/ja/commands"

# ファイル名に使えない字を置き換える
NG = re.compile(r'[/\\:*?"<>|]')


def 名(ラベル: str) -> str:
    return NG.sub("_", ラベル).strip() or "無題"


# コマンド名() と KAZARI は api_taiou にあります(定義は1か所)
コマンド名 = api_taiou.コマンド名


def 一覧():
    """コマンド名ごとに1枚。(ラベル, 段の一覧, 印, officework, 場所, pd, op)

    **同じ名前のボタンは1枚にまとめます**(2026-08-25 発注者「マニュアルの
    コマンドのタイトルはコマンド名にしてまとめないとダメでしょう」)。
    「保存」はファイルとクイックアクセスの両方にありますが、*する事は同じ*
    なので手引きは1枚です。置き場は最初に出てくる段のフォルダで、
    どの段から使えるかは本文の頭に並べます。

    対応表のほうは *両方の行を残します* — あちらは画面を写した物なので、
    2か所にあるボタンは2行あるのが正しい形です。
    """
    まとめ = {}
    for 段, ラベル, _絵, _obj, 印, ow, pd, op in api_taiou.rows():
        # ❌(呼ぶ相手が無い物)にも画面のボタンはありますが、
        # する事が画面の見え方だけなので手引きは要りません
        if 印 == "❌":
            continue
        名前 = コマンド名(ラベル)
        if 名前 in まとめ:
            if 段 not in まとめ[名前][1]:
                まとめ[名前][1].append(段)
            continue
        p = SAKI / 名(段) / f"{名(名前)}.adoc"
        まとめ[名前] = [名前, [段], 印, ow, p, pd, op, ラベル]
    return [tuple(v) for v in まとめ.values()]


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

def 状態の名(ラベル: str, 印: str) -> str:
    """ボタンが使えるかどうか。**印(API の有無)では決めません**

    2026-08-25 発注者「フォルダから探すを ❌ にしたらダメでしょう。api が
    ないというだけでしょう」と同じ間違いを、状態の側でもしていました。
    画像の記入欄は画面から置けるのに、Python の呼び方が無いという理由だけで
    「未実装」と書いていました。

    実物は `face/src/ribbon.rs` の `ready` が持っています。灰色のボタン
    (`ready` が false)が 14 個あり、そこが本当の未実装です。
    リボンに無いボタン(パネルや右クリックだけの物)は、印で決めます。
    """
    if ラベル in _使える():
        return "実装済み"
    if ラベル in _灰色():
        return "未実装"
    # リボンに無いボタン(ファイルのページ・右クリック・シート見出し)は、
    # 実物を読んで確かめた控えで決めます
    if ラベル in api_taiou.HOKA_UGOKU or コマンド名(ラベル) in {
            api_taiou.コマンド名(x) for x in api_taiou.HOKA_UGOKU}:
        return "実装済み"
    return {"✅": "実装済み", "✍": "実装済み"}.get(印, "未実装")


def _リボン():
    import ribbon_parse
    t = ribbon_parse.tables_or_die()
    return [x for 表 in ("WRITER", "CALC") for tab in t[表] for x in tab.cmds]


_使えるの控え = None
_灰色の控え = None


def _使える():
    global _使えるの控え
    if _使えるの控え is None:
        _使えるの控え = {x.label for x in _リボン() if x.ready}
    return _使えるの控え


def _灰色():
    global _灰色の控え
    if _灰色の控え is None:
        使える = _使える()
        _灰色の控え = {x.label for x in _リボン() if not x.ready} - 使える
    return _灰色の控え


下書き = """= {ラベル}

{段}にあります。{同じ}

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
        for ラベル, 段ら, 印, ow, p, pd, op, 画面 in 無い:
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
            st = 状態の名(ラベル, 印)
            同じ = ("" if len(段ら) == 1 else
                    "\n" + "と".join(段ら[1:]) + "にも同じボタンがあります。する事は同じです。")
            # 画面のラベルに飾りが付いているときは、そう見えることを断ります
            if 画面 != ラベル:
                同じ += f"\n画面のボタンは「{画面}」と出ています。"
            p.write_text(下書き.format(ラベル=ラベル, 段=段ら[0], 同じ=同じ,
                                       python=py, 本家=本家,
                                       状態=st, 状態の説明=JOTAI[st],
                                       一言=一言(ラベル),
                                       ダイアログ="(ダイアログは出ません)" if 印 == "✅"
                                       else "(ダイアログが出るときは、選ぶものをここに書きます)"),
                        encoding="utf-8")
        print(f"{len(無い)} 枚の下書きを作りました")
        return 0

    if "--index" in sys.argv:
        o = ["= コマンドの手引き", "",
             "*コマンド1つに1枚*です。段ごとに並べてあります。", "",
             "同じ名前のボタンが2か所にあるときは、手引きは1枚です。"
             "どちらの段からも同じ1枚に飛びます。", ""]
        # 段の順は対応表と同じ。1枚が2つの段に出ることがあります
        段の順, 中身 = [], {}
        for ラベル, 段ら, 印, ow, p, *_ in r:
            for 段 in 段ら:
                if 段 not in 中身:
                    段の順.append(段)
                    中身[段] = []
                中身[段].append((ラベル, p))
        for 段 in 段の順:
            o += [f"== {段}", ""]
            for ラベル, p in 中身[段]:
                if p.exists():
                    先 = p.relative_to(SAKI).as_posix()
                    o.append(f"* link:{先}[{ラベル}]")
                else:
                    o.append(f"* {ラベル}(まだ)")
            o.append("")
        SAKI.mkdir(parents=True, exist_ok=True)
        (SAKI / "README.adoc").write_text("\n".join(o) + "\n", encoding="utf-8")
        print(f"目次を書きました({len(r)} 項目)")
        return 0

    # **状態ごとに数えます。** 手引きは実物より先に書くので、
    # 「未実装」の枚数がこれから作る物の一覧になります
    数, ずれ = {}, []
    for q in sorted(SAKI.rglob("*.adoc")):
        if q.name == "README.adoc":
            continue
        s = q.read_text(encoding="utf-8")
        m = re.search(r"\*状態: (実装済み|未実装|廃止予定)\*", s)
        いま = m.group(1) if m else "印が無い"
        数[いま] = 数.get(いま, 0) + 1
        # **書いた状態が実物と合っているか。** ファイル名でなく見出しで引きます
        # (`ヘッダー_フッター.adoc` の名前は `ヘッダー/フッター` です)
        見 = re.match(r"= (.+)", s)
        if m and 見 and いま != "廃止予定":
            名前 = 見.group(1).strip()
            印 = next((x[4] for x in api_taiou.rows()
                       if コマンド名(x[1]) == 名前), "")
            べき = 状態の名(名前, 印)
            if べき != いま:
                ずれ.append((q.relative_to(ROOT).as_posix(), いま, べき))
    print(f"手引きが要るボタン {len(r)} 枚のうち、書けているのは {len(r) - len(無い)} 枚です")
    print("書いた手引きの状態:")
    for k in ("実装済み", "未実装", "廃止予定", "印が無い"):
        if 数.get(k):
            print(f"  {k:<8} {数[k]} 枚")
    # 表から消えたのに残っている枚。まとめ直した後の後片づけに要ります
    要る = {x[4].resolve() for x in r}
    余り = [q for q in sorted(SAKI.rglob("*.adoc"))
            if q.name != "README.adoc" and q.resolve() not in 要る]
    if 余り:
        print(f"\n表に無い手引きが {len(余り)} 枚あります(消してください):")
        for q in 余り:
            print(f"  {q.relative_to(ROOT)}")

    if ずれ:
        print(f"\n**状態が実物と違う手引きが {len(ずれ)} 枚あります。**", file=sys.stderr)
        print("ボタンが動くかは face/src/ribbon.rs の ready と、"
              "api_taiou.HOKA_UGOKU が持っています。\n", file=sys.stderr)
        for f, a, b in ずれ:
            print(f"  {f}: 「{a}」と書いてありますが「{b}」です", file=sys.stderr)
        return 1

    if 無い:
        print("\nまだ無いもの(段ごとの数):")
        から = {}
        for _ラベル, 段ら, *_ in 無い:
            段 = 段ら[0]
            から[段] = から.get(段, 0) + 1
        for 段, n in から.items():
            print(f"  {段:<24} {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
