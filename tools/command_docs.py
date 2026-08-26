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


def name(label: str) -> str:
    return NG.sub("_", label).strip() or "無題"


# コマンド名() と KAZARI は api_taiou にあります(定義は1か所)
command_name = api_taiou.command_name


def list():
    """コマンド名ごとに1枚。(ラベル, 段の一覧, 印, officework, 場所, pd, op)

    **同じ名前のボタンは1枚にまとめます**(2026-08-25 発注者「マニュアルの
    コマンドのタイトルはコマンド名にしてまとめないとダメでしょう」)。
    「保存」はファイルとクイックアクセスの両方にありますが、*する事は同じ*
    なので手引きは1枚です。置き場は最初に出てくる段のフォルダで、
    どの段から使えるかは本文の頭に並べます。

    対応表のほうは *両方の行を残します* — あちらは画面を写した物なので、
    2か所にあるボタンは2行あるのが正しい形です。
    """
    summary = {}
    for tab, label, _icon, _obj, mark, ow, pd, op in api_taiou.rows():
        # ❌(呼ぶ相手が無い物)にも画面のボタンはありますが、
        # する事が画面の見え方だけなので手引きは要りません
        if mark == "❌":
            continue
        member = command_name(label)
        if member in summary:
            if tab not in summary[member][1]:
                summary[member][1].append(tab)
            continue
        p = SAKI / name(tab) / f"{name(member)}.adoc"
        summary[member] = [member, [tab], mark, ow, p, pd, op, label]
    return [tuple(v) for v in summary.values()]


# 状態。**手引きは実物より先に書きます**(2026-08-24 発注者)。
# 画面の灰色のボタンと同じ考え方で、*あることは見せて、状態で言う*。
JOTAI = {
    "実装済み": "いま使えます",
    "未実装": "*まだ使えません。* これから作ります",
    "廃止予定": "*なくす予定です。* 行き先を下に書いてあります",
}

# ボタンの名前 → キー(`face/src/keys.rs` と `keys_doc.py` の説明から結ぶ)
def _key_table():
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


def one_word(label: str) -> str:
    for member, sentence in KUBUN:
        if label in member:
            return sentence
    return "(まだ書いていません)"


# 状態。**手引きは実物より先に書きます**(2026-08-24 発注者)。
# 画面の灰色のボタンと同じ考え方で、*あることは見せて、状態で言う*。
JOTAI = {
    "実装済み": "いま使えます",
    "未実装": "*まだ使えません。* これから作ります",
    "廃止予定": "*なくす予定です。* 行き先を下に書いてあります",
}

def state_name(label: str, mark: str) -> str:
    """ボタンが使えるかどうか。**印(API の有無)では決めません**

    2026-08-25 発注者「フォルダから探すを ❌ にしたらダメでしょう。api が
    ないというだけでしょう」と同じ間違いを、状態の側でもしていました。
    画像の記入欄は画面から置けるのに、Python の呼び方が無いという理由だけで
    「未実装」と書いていました。

    実物は `face/src/ribbon.rs` の `ready` が持っています。灰色のボタン
    (`ready` が false)が 14 個あり、そこが本当の未実装です。
    リボンに無いボタン(パネルや右クリックだけの物)は、印で決めます。
    """
    if label in _ready():
        return "実装済み"
    if label in _grey():
        return "未実装"
    # リボンに無いボタン(ファイルのページ・右クリック・シート見出し)は、
    # 実物を読んで確かめた控えで決めます
    if label in api_taiou.HOKA_UGOKU or command_name(label) in {
            api_taiou.command_name(x) for x in api_taiou.HOKA_UGOKU}:
        return "実装済み"
    return {"✅": "実装済み", "✍": "実装済み"}.get(mark, "未実装")


def _ribbon():
    import ribbon_parse
    t = ribbon_parse.tables_or_die()
    return [x for table in ("WRITER", "CALC") for tab in t[table] for x in tab.cmds]


_ready_cache = None
_grey_cache = None


def _ready():
    global _ready_cache
    if _ready_cache is None:
        _ready_cache = {x.label for x in _ribbon() if x.ready}
    return _ready_cache


def _grey():
    global _grey_cache
    if _grey_cache is None:
        usable = _ready()
        _grey_cache = {x.label for x in _ribbon() if not x.ready} - usable
    return _grey_cache


draft_of = """= {label}

{tab}にあります。{same}

*状態: {state}* — {state_desc}

== 何をするか

{one_word}

== ダイアログ

{dialog}

== プログラムから

{python}
{vendor}
"""


def main() -> int:
    r = list()
    missing = [x for x in r if not x[4].exists()]

    if "--make" in sys.argv:
        for label, tabs_of, mark, ow, p, pd, op, screen in missing:
            p.parent.mkdir(parents=True, exist_ok=True)
            if mark == "✍":
                writing = ow or api_taiou.reason(label, "✍") or ""
                py = ("専用の呼び方はありません。こう書けば同じことができます。\n\n"
                      f"[source,python]\n----\n{writing}\n----")
            elif ow:
                py = f"[source,python]\n----\n{ow}\n----"
            else:
                py = "(まだありません)"
            vendor = ""
            src_of = [x for x in ((pd, "python-docx"), (op, "openpyxl"))
                    if x[0] and x[0] != "—"]
            if src_of:
                vendor = "\n他のライブラリでは、こう書きます。\n\n[cols=\"1,2\"]\n|===\n|道具 |書き方\n\n"
                vendor += "\n".join(f"|{name} |`{writ}`" for writ, name in src_of) + "\n|===\n"
            st = state_name(label, mark)
            same = ("" if len(tabs_of) == 1 else
                    "\n" + "と".join(tabs_of[1:]) + "にも同じボタンがあります。する事は同じです。")
            # 画面のラベルに飾りが付いているときは、そう見えることを断ります
            if screen != label:
                same += f"\n画面のボタンは「{screen}」と出ています。"
            p.write_text(draft_of.format(label=label, tab=tabs_of[0], same=same,
                                       python=py, vendor=vendor,
                                       state=st, state_desc=JOTAI[st],
                                       one_word=one_word(label),
                                       dialog="(ダイアログは出ません)" if mark == "✅"
                                       else "(ダイアログが出るときは、選ぶものをここに書きます)"),
                        encoding="utf-8")
        print(f"{len(missing)} 枚の下書きを作りました")
        return 0

    if "--index" in sys.argv:
        o = ["= コマンドの手引き", "",
             "*コマンド1つに1枚*です。段ごとに並べてあります。", "",
             "同じ名前のボタンが2か所にあるときは、手引きは1枚です。"
             "どちらの段からも同じ1枚に飛びます。", ""]
        # 段の順は対応表と同じ。1枚が2つの段に出ることがあります
        tab_order, content = [], {}
        for label, tabs_of, mark, ow, p, *_ in r:
            for tab in tabs_of:
                if tab not in content:
                    tab_order.append(tab)
                    content[tab] = []
                content[tab].append((label, p))
        for tab in tab_order:
            o += [f"== {tab}", ""]
            for label, p in content[tab]:
                if p.exists():
                    to = p.relative_to(SAKI).as_posix()
                    o.append(f"* link:{to}[{label}]")
                else:
                    o.append(f"* {label}(まだ)")
            o.append("")
        SAKI.mkdir(parents=True, exist_ok=True)
        (SAKI / "README.adoc").write_text("\n".join(o) + "\n", encoding="utf-8")
        print(f"目次を書きました({len(r)} 項目)")
        return 0

    # **状態ごとに数えます。** 手引きは実物より先に書くので、
    # 「未実装」の枚数がこれから作る物の一覧になります
    numbers, drift = {}, []
    for q in sorted(SAKI.rglob("*.adoc")):
        if q.name == "README.adoc":
            continue
        s = q.read_text(encoding="utf-8")
        m = re.search(r"\*状態: (実装済み|未実装|廃止予定)\*", s)
        current = m.group(1) if m else "印が無い"
        numbers[current] = numbers.get(current, 0) + 1
        # **書いた状態が実物と合っているか。** ファイル名でなく見出しで引きます
        # (`ヘッダー_フッター.adoc` の名前は `ヘッダー/フッター` です)
        look = re.match(r"= (.+)", s)
        if m and look and current != "廃止予定":
            member = look.group(1).strip()
            mark = next((x[4] for x in api_taiou.rows()
                       if command_name(x[1]) == member), "")
            should_be = state_name(member, mark)
            if should_be != current:
                drift.append((q.relative_to(ROOT).as_posix(), current, should_be))
    print(f"手引きが要るボタン {len(r)} 枚のうち、書けているのは {len(r) - len(missing)} 枚です")
    print("書いた手引きの状態:")
    for k in ("実装済み", "未実装", "廃止予定", "印が無い"):
        if numbers.get(k):
            print(f"  {k:<8} {numbers[k]} 枚")
    # 表から消えたのに残っている枚。まとめ直した後の後片づけに要ります
    needed = {x[4].resolve() for x in r}
    remainder = [q for q in sorted(SAKI.rglob("*.adoc"))
            if q.name != "README.adoc" and q.resolve() not in needed]
    if remainder:
        print(f"\n表に無い手引きが {len(remainder)} 枚あります(消してください):")
        for q in remainder:
            print(f"  {q.relative_to(ROOT)}")

    if drift:
        print(f"\n**状態が実物と違う手引きが {len(drift)} 枚あります。**", file=sys.stderr)
        print("ボタンが動くかは face/src/ribbon.rs の ready と、"
              "api_taiou.HOKA_UGOKU が持っています。\n", file=sys.stderr)
        for f, a, b in drift:
            print(f"  {f}: 「{a}」と書いてありますが「{b}」です", file=sys.stderr)
        return 1

    if missing:
        print("\nまだ無いもの(段ごとの数):")
        from_of = {}
        for _label, tabs_of, *_ in missing:
            tab = tabs_of[0]
            from_of[tab] = from_of.get(tab, 0) + 1
        for tab, n in from_of.items():
            print(f"  {tab:<24} {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
