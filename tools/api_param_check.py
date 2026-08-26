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


def 引く(name: str):
    """`officework.doc.Doc` のような名前を引く。無ければ None"""
    src_of = CHOKU.get(name)
    if src_of is None:
        return None
    return _実物(src_of)


def 受け手(text: str):
    """その字が指す物を全部返す。どれか1つにあれば合っています"""
    return [x for x in (_実物(y) for y in UKETE.get(text, [])) if x is not None]


def _引き当てる(name: str):
    """`officework.doc.Doc` を引く。

    **`officework.doc` は import できません。** 包みの中の属性であって、
    サブモジュールではないからです。import できる所まで import して、
    残りは属性で辿ります(2026-08-25 — ここを間違えて officework の
    88 行を1つも見ないまま「全部合っています」と出していました)。
    """
    節 = name.split(".")
    item, i = None, 0
    for i in range(len(節), 0, -1):
        try:
            item = importlib.import_module(".".join(節[:i]))
            break
        except ImportError:
            continue
        except SystemExit as e:
            # 足りない物を告げて止まる包みがあります(officework.mcp は
            # `mcp` が無いと SystemExit します)。飲み込まずに控えます
            未確認.add(f"{name} — {e}")
            return None
    if item is None:
        return None
    for member in 節[i:]:
        item = getattr(item, member, None)
        if item is None:
            return None
    return item


def _実物(src_of):
    mod, attr = src_of
    C = _引き当てる(f"{mod}.{attr}")
    if C is None:
        return None
    作る = MIHON.get(src_of)
    if 作る is None:
        return C
    try:
        return 作る(C)
    except Exception:
        return C        # 作れなければクラスのまま見ます


# 呼び方1つ。`受け手.名前(引数)` か `受け手.名前` の形
YOBI = re.compile(r"^([A-Za-z_][\w.]*)\.(\w+)(\((.*)\))?$")


def 割る(form: str):
    """`A / B` で並べてあるものを1つずつに割る。

    `(col_span / v_merge) / s.merge_cells(…)` のように括弧の中に
    `/` が入ることがあるので、*括弧の外の `/`* だけで割ります。
    """
    out, depth, current = [], 0, ""
    for ch in form:
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        if ch == "/" and depth == 0:
            out.append(current)
            current = ""
        else:
            current += ch
    out.append(current)
    return [x.strip() for x in out if x.strip()]


def _代入の左(form: str) -> str:
    """括弧の外に `=` があれば、その左を返す。無ければそのまま"""
    depth = 0
    for i, ch in enumerate(form):
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        elif ch == "=" and depth == 0:
            if form[i:i + 2] == "==" or (i and form[i - 1] in "=<>!"):
                return form.strip()
            return form[:i].strip()
    return form.strip()


def 数える(args: str):
    """`前, 後` から (位置の数, キーワードの名前) を出す。

    `…` は「ここに何か入る」の印なので、数えません。
    """
    if args is None or not args.strip():
        return 0, []
    positions, keys = 0, []
    for x in args.split(","):
        x = x.strip()
        if not x or x == "…":
            continue
        m = re.match(r"^(\w+)=", x)
        if m:
            keys.append(m.group(1))
        else:
            positions += 1
    return positions, keys


def check(form: str, どこ: str, bad: list):
    for 一つ in 割る(form):
        # 代入の左だけ見ます(`p.style = '箇条書き'` は p.style を見る)。
        # **括弧の外の `=` だけ**です。`d.render(値, rows=行)` の `=` は
        # キーワード引数なので、ここで切ると呼び方が壊れて素通りします
        left = _代入の左(一つ)
        if left.startswith("(") or left in ("—", ""):
            continue
        m = YOBI.match(left)
        if not m:
            continue
        受, member, _かっこ, args = m.groups()
        cands = [x for x in [引く(受)] if x is not None] or 受け手(受)
        if not cands:
            # **知らない字は控えます。** 飛ばした物を黙っていると、
            # 何も見ないまま「合っています」と出ます
            if 受 in UKETE or 受 in CHOKU:
                未確認.add(f"{受}({どこ})")
            continue
        あった = [x for x in cands if hasattr(x, member)]
        if not あった:
            name = "・".join(sorted({_名(x) for x in cands}))
            bad.append((どこ, left, f"{受}({name})に {member} がありません"))
            continue
        if args is None:
            continue        # 属性を読むだけ。数える物がありません
        # **どれか1つで通ればよい**ので、全部の言い分を集めてから決めます
        notes = [_数の検査(x, member, args, 受) for x in あった]
        if any(v is None for v in notes):
            continue
        bad.append((どこ, left, notes[0]))


def _名(item) -> str:
    return item.__name__ if isinstance(item, type) else type(item).__name__


def _数の検査(item, member: str, args: str, 受: str):
    """合っていれば None、ずれていれば理由の字を返す"""
    content = getattr(item, member)
    if not callable(content):
        return f"{member} は呼べません(属性です)"
    try:
        署名 = inspect.signature(content)
    except (TypeError, ValueError):
        return None         # C で書いた物は署名が取れません
    positions, keys = 数える(args)
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
    if positions > 受ける:
        return f"{positions} 個渡していますが、{member} が受けるのは {受ける} 個です"
    if "…" not in args and positions + len(keys) < 必須:
        return f"{positions + len(keys)} 個渡していますが、{member} は {必須} 個要ります"
    for k in keys:
        if k not in 鍵の名:
            return f"{member} に {k}= という引数はありません"
    return None


def main() -> int:
    bad = []
    seen = 0
    for tab, label, _絵, _obj, mark, ow, pd, op in api_taiou.rows():
        if mark != "✅" or not ow:
            continue
        seen += 1
        check(ow, f"{tab}/{label}", bad)
        for x in (pd, op):
            if x and x != "—":
                check(x, f"{tab}/{label}", bad)
    if 未確認:
        print(f"**{len(未確認)} 件の受け取り手が引けませんでした。**", file=sys.stderr)
        print("引けないまま通すと、何も見ないで「合っています」と出ます。"
              "足りない包みを入れてください。\n", file=sys.stderr)
        for x in sorted(未確認):
            print(f"  {x}", file=sys.stderr)
        return 1
    if not bad:
        print(f"対応表の ✅ {seen} 行、呼び方は全部合っています")
        return 0
    print(f"**呼び方が {len(bad)} 箇所ずれています。**", file=sys.stderr)
    print("表を直すか、実物を作ってください"
          "(まだ無い物は ✅ ではなく空にします)。\n", file=sys.stderr)
    for どこ, 書き, なぜ in bad:
        print(f"  {どこ}", file=sys.stderr)
        print(f"      {書き} — {なぜ}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
