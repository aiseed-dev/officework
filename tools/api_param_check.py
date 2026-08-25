#!/usr/bin/env python3
"""対応表に書いた呼び方が、本物と合っているかを確かめる。

    python3 tools/api_param_check.py

発注者 2026-08-25「パラメータをきちんとチェックする」。

対応表(`tools/api_taiou.py`)の `officework` の列には
`Doc.open(径路)` や `d.replace(前, 後)` のような呼び方が並んでいます。
**手で書いた字なので、実物と離れても誰も気づきません。**
この道具は、書いてある呼び方を実際に引いて確かめます。

見るのは3つです。

. その名前が本当にあるか(`Doc.render` が無いのに ✅ になっていないか)
. 渡している数が合っているか(`d.replace(前, 後)` は2つ受け取るか)
. キーワードの名前が合っているか(`rows=` という引数があるか)

✅ の行だけ見ます。✍ と空と ❌ は、まだ呼び方が無いか、呼ぶ相手が
ありません。本家(python-docx / openpyxl)の列は、入っていれば一緒に見ます。

**受け取り手の字は決めてあります。** `d` は Doc、`b` は Book、
`p` は Paragraph、`r` は Run、`c` は Cell、`s` は Sheet、`t` は Table です。
"""
import ast
import importlib
import inspect
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).parent))
# **綴りの中身を見ます。** `.venv` に入れた写しは古いことがあります
# (2026-08-25 に実際、mcp.py の入っていない写しを見て「無い」と出しました)
sys.path.insert(0, str(ROOT / "pysheet"))
import api_taiou  # noqa: E402

# 受け取り手の字 → 引く先。**1つの字が2つを指すことがあります**。
# `c` は文書の表のセルにも、シートのセルにも使います。
# どちらかにあれば合っている、と見ます
UKETE = {
    "d": [("officework.doc", "Doc")],
    "b": [("officework.sheet", "Book")],
    "p": [("officework.doc", "Paragraph")],
    "r": [("officework.doc", "Run")],
    "c": [("officework.sheet", "Cell"), ("officework.doc", "Cell")],
    "s": [("officework.sheet", "Sheet")],
    "t": [("officework.doc", "Table"), ("officework.sheet", "Table")],
    "ws": [("openpyxl.worksheet.worksheet", "Worksheet")],
    "wb": [("openpyxl.workbook", "Workbook")],
    "cell": [("docx.table", "_Cell")],
    "section": [("docx.section", "Section")],
    "mcp": [("officework", "mcp")],
}
# **実物を作ってから引くもの。**
# `Workbook.properties` のように `__init__` の中で足す属性は、
# クラスを引いても出てきません(2026-08-25 に8件これで誤って落ちました)
MIHON = {
    ("openpyxl.workbook", "Workbook"): lambda C: C(),
    ("openpyxl.worksheet.worksheet", "Worksheet"): lambda C: __import__(
        "openpyxl").Workbook().active,
}

# 名前で直に書いてあるもの
CHOKU = {
    "Doc": ("officework.doc", "Doc"),
    "Book": ("officework.sheet", "Book"),
    "docx.Document": ("docx", "Document"),
    "load_workbook": ("openpyxl", "load_workbook"),
    "Workbook": ("openpyxl", "Workbook"),
}

# 値の見本。字の意味は問わないので、数と名前だけ見ます
_値 = re.compile(r"^[^=]+$")


# 引けなかった受け取り手。**黙って飛ばすと「全部合っています」と出ます**
# (2026-08-25 — officework が引けていないのに 88 行合格と出しました)
未確認 = set()


def 引く(名: str):
    """`officework.doc.Doc` のような名前を引く。無ければ None"""
    もと = CHOKU.get(名)
    if もと is None:
        return None
    return _実物(もと)


def 受け手(字: str):
    """その字が指す物を全部返す。どれか1つにあれば合っています"""
    return [x for x in (_実物(y) for y in UKETE.get(字, [])) if x is not None]


def _引き当てる(名: str):
    """`officework.doc.Doc` を引く。

    **`officework.doc` は import できません。** 包みの中の属性であって、
    サブモジュールではないからです。import できる所まで import して、
    残りは属性で辿ります(2026-08-25 — ここを間違えて officework の
    88 行を1つも見ないまま「全部合っています」と出していました)。
    """
    節 = 名.split(".")
    もの, i = None, 0
    for i in range(len(節), 0, -1):
        try:
            もの = importlib.import_module(".".join(節[:i]))
            break
        except ImportError:
            continue
        except SystemExit as e:
            # 足りない物を告げて止まる包みがあります(officework.mcp は
            # `mcp` が無いと SystemExit します)。飲み込まずに控えます
            未確認.add(f"{名} — {e}")
            return None
    if もの is None:
        return None
    for 名前 in 節[i:]:
        もの = getattr(もの, 名前, None)
        if もの is None:
            return None
    return もの


def _実物(もと):
    mod, attr = もと
    C = _引き当てる(f"{mod}.{attr}")
    if C is None:
        return None
    作る = MIHON.get(もと)
    if 作る is None:
        return C
    try:
        return 作る(C)
    except Exception:
        return C        # 作れなければクラスのまま見ます


# 呼び方1つ。`受け手.名前(引数)` か `受け手.名前` の形
YOBI = re.compile(r"^([A-Za-z_][\w.]*)\.(\w+)(\((.*)\))?$")


def 割る(書き方: str):
    """`A / B` で並べてあるものを1つずつに割る。

    `(col_span / v_merge) / s.merge_cells(…)` のように括弧の中に
    `/` が入ることがあるので、*括弧の外の `/`* だけで割ります。
    """
    出, 深さ, いま = [], 0, ""
    for ch in 書き方:
        if ch in "([":
            深さ += 1
        elif ch in ")]":
            深さ -= 1
        if ch == "/" and 深さ == 0:
            出.append(いま)
            いま = ""
        else:
            いま += ch
    出.append(いま)
    return [x.strip() for x in 出 if x.strip()]


def _代入の左(書き方: str) -> str:
    """括弧の外に `=` があれば、その左を返す。無ければそのまま"""
    深さ = 0
    for i, ch in enumerate(書き方):
        if ch in "([":
            深さ += 1
        elif ch in ")]":
            深さ -= 1
        elif ch == "=" and 深さ == 0:
            if 書き方[i:i + 2] == "==" or (i and 書き方[i - 1] in "=<>!"):
                return 書き方.strip()
            return 書き方[:i].strip()
    return 書き方.strip()


def 数える(引数: str):
    """`前, 後` から (位置の数, キーワードの名前) を出す。

    `…` は「ここに何か入る」の印なので、数えません。
    """
    if 引数 is None or not 引数.strip():
        return 0, []
    位置, 鍵 = 0, []
    for x in 引数.split(","):
        x = x.strip()
        if not x or x == "…":
            continue
        m = re.match(r"^(\w+)=", x)
        if m:
            鍵.append(m.group(1))
        else:
            位置 += 1
    return 位置, 鍵


def 見る(書き方: str, どこ: str, 悪い: list):
    for 一つ in 割る(書き方):
        # 代入の左だけ見ます(`p.style = '箇条書き'` は p.style を見る)。
        # **括弧の外の `=` だけ**です。`d.render(値, rows=行)` の `=` は
        # キーワード引数なので、ここで切ると呼び方が壊れて素通りします
        左 = _代入の左(一つ)
        if 左.startswith("(") or 左 in ("—", ""):
            continue
        m = YOBI.match(左)
        if not m:
            continue
        受, 名前, _かっこ, 引数 = m.groups()
        候補 = [x for x in [引く(受)] if x is not None] or 受け手(受)
        if not 候補:
            # **知らない字は控えます。** 飛ばした物を黙っていると、
            # 何も見ないまま「合っています」と出ます
            if 受 in UKETE or 受 in CHOKU:
                未確認.add(f"{受}({どこ})")
            continue
        あった = [x for x in 候補 if hasattr(x, 名前)]
        if not あった:
            名 = "・".join(sorted({_名(x) for x in 候補}))
            悪い.append((どこ, 左, f"{受}({名})に {名前} がありません"))
            continue
        if 引数 is None:
            continue        # 属性を読むだけ。数える物がありません
        # **どれか1つで通ればよい**ので、全部の言い分を集めてから決めます
        言い分 = [_数の検査(x, 名前, 引数, 受) for x in あった]
        if any(v is None for v in 言い分):
            continue
        悪い.append((どこ, 左, 言い分[0]))


def _名(もの) -> str:
    return もの.__name__ if isinstance(もの, type) else type(もの).__name__


def _数の検査(もの, 名前: str, 引数: str, 受: str):
    """合っていれば None、ずれていれば理由の字を返す"""
    中身 = getattr(もの, 名前)
    if not callable(中身):
        return f"{名前} は呼べません(属性です)"
    try:
        署名 = inspect.signature(中身)
    except (TypeError, ValueError):
        return None         # C で書いた物は署名が取れません
    位置, 鍵 = 数える(引数)
    受ける, 何でも, 鍵の名, 必須 = 0, False, set(), 0
    for prm in 署名.parameters.values():
        if prm.name == "self":
            continue
        if prm.kind in (prm.VAR_POSITIONAL, prm.VAR_KEYWORD):
            何でも = True
        elif prm.kind is prm.KEYWORD_ONLY:
            鍵の名.add(prm.name)
        else:
            受ける += 1
            鍵の名.add(prm.name)
            if prm.default is prm.empty:
                必須 += 1
    if 何でも:
        return None
    if 位置 > 受ける:
        return f"{位置} 個渡していますが、{名前} が受けるのは {受ける} 個です"
    if "…" not in 引数 and 位置 + len(鍵) < 必須:
        return f"{位置 + len(鍵)} 個渡していますが、{名前} は {必須} 個要ります"
    for k in 鍵:
        if k not in 鍵の名:
            return f"{名前} に {k}= という引数はありません"
    return None


def main() -> int:
    悪い = []
    見た = 0
    for 段, ラベル, _絵, _obj, 印, ow, pd, op in api_taiou.rows():
        if 印 != "✅" or not ow:
            continue
        見た += 1
        見る(ow, f"{段}/{ラベル}", 悪い)
        for x in (pd, op):
            if x and x != "—":
                見る(x, f"{段}/{ラベル}", 悪い)
    if 未確認:
        print(f"**{len(未確認)} 件の受け取り手が引けませんでした。**", file=sys.stderr)
        print("引けないまま通すと、何も見ないで「合っています」と出ます。"
              "足りない包みを入れてください。\n", file=sys.stderr)
        for x in sorted(未確認):
            print(f"  {x}", file=sys.stderr)
        return 1
    if not 悪い:
        print(f"対応表の ✅ {見た} 行、呼び方は全部合っています")
        return 0
    print(f"**呼び方が {len(悪い)} 箇所ずれています。**", file=sys.stderr)
    print("表を直すか、実物を作ってください"
          "(まだ無い物は ✅ ではなく空にします)。\n", file=sys.stderr)
    for どこ, 書き, なぜ in 悪い:
        print(f"  {どこ}", file=sys.stderr)
        print(f"      {書き} — {なぜ}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
