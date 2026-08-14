"""`face/src/ribbon.rs` の表を読む。**この repo で唯一の読み手。**

5つの道具がそれぞれ正規表現で `ribbon.rs` を読んでいた(`wiring_check` /
`ribbon_locale_check` に3つ / `gray_claim_check` / `ribbon_sweep` /
`gen_ribbon_locale`)。**全部が「合致する物を拾う」形**で、拾えなかった物は
黙って無かったことになる。書き方が変われば静かに減り、`FLOOR` は全盲だけを
拾って半分になった状態は素通りする(2026-08-12、fork セッションが数えた)。

## だから「拾う」をやめて「食べ尽くす」

**領域の中身を全部消費し、残りが1文字でも出たら落ちる。**

合致を探す形は黙って飛ばせるが、食べ尽くす形は飛ばせない。知らない書き方が
入った瞬間、それは「残り」として出てくる。**読めなくなったことを、読めた
つもりで通さない。**

いまの `ribbon.rs` は完全に規則的で、この解析器で残り 0 文字になる。
つまり**今日そのまま緑で、誰かが新しい書き方をした日に赤くなる。**

## 落とす条件

1. `pub const CALC/WRITER: &[Tab] = &[ … ];` が見つからない
2. 消費し終えて**残りがある** — 残った字と行番号を出す
3. タブが0個、または中身0個のタブがある
4. `/* … */` がある — **いま扱えないので、扱えないと言って止める**
   (どの解析器も `//` しか落としていなかったので、`/* */` で囲んだボタンは
   **生きたボタンとして数えられていた**。黙って誤読するより止まるほうがよい)

使い方:

    from ribbon_parse import tables, RibbonError
    t = tables()                    # {"CALC": [Tab, …], "WRITER": [Tab, …]}
    ids = [c.id for tab in t["CALC"] for c in tab.cmds if c.kind == "c"]

呼ぶ側で `::error::` を出して終わりたいなら `tables_or_die()`。
"""

from __future__ import annotations

import pathlib
import re
import sys
from typing import NamedTuple

ROOT = pathlib.Path(__file__).resolve().parents[1]
RIBBON = ROOT / "face/src/ribbon.rs"


class Cmd(NamedTuple):
    """1つのボタン。`kind` は `"c"`(押せる)か `"x"`(灰色)。

    **灰色に id は無い** — `x(label, icon)` の2引数で、実物も `id: ""` を入れる。
    """

    kind: str
    id: str | None
    label: str
    icon: str

    @property
    def ready(self) -> bool:
        return self.kind == "c"


class Tab(NamedTuple):
    name: str
    cmds: list[Cmd]


class RibbonError(Exception):
    """読めなかった。**黙って空を返さない**ための例外。"""


# 文字列は `\"` の逃げを許す(いまの表には出てこないが、出たときに
# 「読めない」ではなく正しく読めるほうがよい)
_STR = r'"((?:[^"\\]|\\.)*)"'
_WS = re.compile(r"\s+")
_LINE_COMMENT = re.compile(r"//[^\n]*")
_TAB_OPEN = re.compile(r"Tab\s*\{\s*name:\s*" + _STR + r"\s*,\s*cmds:\s*&\[")
_TAB_CLOSE = re.compile(r"\]\s*\}")
_C = re.compile(r"c\(\s*" + _STR + r"\s*,\s*" + _STR + r"\s*,\s*" + _STR + r"\s*,?\s*\)")
_X = re.compile(r"x\(\s*" + _STR + r"\s*,\s*" + _STR + r"\s*,?\s*\)")
_COMMA = re.compile(r",")


def _table_region(src: str, name: str) -> tuple[int, int]:
    """`pub const NAME: &[Tab] = &[` から、対応する `];` までを返す。

    **括弧を数える。** 終わりを `^];` の字面で探すと、字下げの流儀が変わった
    だけで見失う。
    """
    head = re.search(rf"pub const {name}\s*:\s*&\[Tab\]\s*=\s*&\[", src)
    if not head:
        raise RibbonError(f"{name} の表が見つかりません(`pub const {name}: &[Tab] = &[` の形が変わった?)")
    i = head.end()
    depth = 1
    while i < len(src):
        ch = src[i]
        if ch == '"':                       # 文字列の中の括弧は数えない
            m = re.compile(_STR).match(src, i)
            if not m:
                raise RibbonError(f"{name}: 閉じていない文字列があります({_line_of(src, i)} 行目)")
            i = m.end()
            continue
        if ch == "[":
            depth += 1
        elif ch == "]":
            depth -= 1
            if depth == 0:
                return head.end(), i
        i += 1
    raise RibbonError(f"{name}: 表が閉じていません")


def _line_of(src: str, pos: int) -> int:
    return src.count("\n", 0, pos) + 1


def _parse_table(src: str, name: str) -> list[Tab]:
    a, b = _table_region(src, name)
    body = src[a:b]
    base = a

    # 行の注釈だけ落とす。**`/* */` は落とさない** — 落とすと「囲んで消した
    # ボタン」を静かに見逃す形になるので、下で見つけて止める
    if "/*" in body:
        raise RibbonError(
            f"{name}: `/* … */` があります({_line_of(src, base + body.index('/*'))} 行目)。"
            "この解析器は扱えません — **黙って誤読するより止まります**。"
            "囲んで消すのではなく `x(…)` で灰色にしてください"
        )
    body = _LINE_COMMENT.sub(lambda m: " " * len(m.group(0)), body)

    tabs: list[Tab] = []
    cur: list[Cmd] | None = None
    i = 0
    n = len(body)
    while i < n:
        if m := _WS.match(body, i):
            i = m.end()
            continue
        if m := _COMMA.match(body, i):
            i = m.end()
            continue
        if cur is None:
            if m := _TAB_OPEN.match(body, i):
                cur = []
                tabs.append(Tab(_unescape(m.group(1)), cur))
                i = m.end()
                continue
        else:
            if m := _C.match(body, i):
                cur.append(Cmd("c", _unescape(m.group(1)), _unescape(m.group(2)), _unescape(m.group(3))))
                i = m.end()
                continue
            if m := _X.match(body, i):
                cur.append(Cmd("x", None, _unescape(m.group(1)), _unescape(m.group(2))))
                i = m.end()
                continue
            if m := _TAB_CLOSE.match(body, i):
                cur = None
                i = m.end()
                continue
        # **ここに来たら負け。** 知らない書き方が入っている
        rest = body[i : i + 60].strip().splitlines()[0] if body[i:].strip() else ""
        raise RibbonError(
            f"{name}: {_line_of(src, base + i)} 行目から読めません: {rest!r}\n"
            "  **合致しなかった物を飛ばさずに止めています。** 表の書き方を変えたなら "
            "tools/ribbon_parse.py も直してください(5つの道具が全部ここを通ります)"
        )
    if cur is not None:
        raise RibbonError(f"{name}: タブが閉じていません")
    if not tabs:
        raise RibbonError(f"{name}: タブが1つもありません")
    for t in tabs:
        if not t.cmds:
            raise RibbonError(f"{name}: タブ「{t.name}」の中身が空です")
    return tabs


def _unescape(s: str) -> str:
    return s.replace('\\"', '"').replace("\\\\", "\\")


def tables(path: pathlib.Path | str = RIBBON) -> dict[str, list[Tab]]:
    """`{"CALC": [Tab, …], "WRITER": [Tab, …]}`。読めなければ `RibbonError`。"""
    src = pathlib.Path(path).read_text(encoding="utf-8")
    others = [m for m in re.findall(r"pub const (\w+)\s*:\s*&\[Tab\]", src) if m not in ("CALC", "WRITER")]
    if others:
        # **知らない表が増えたら言う。** 黙って CALC/WRITER だけ見ると、
        # 新しい表が誰にも検査されないまま入る
        raise RibbonError(f"知らない表があります: {', '.join(others)}(道具の対象に足してください)")
    return {name: _parse_table(src, name) for name in ("CALC", "WRITER")}


def tables_or_die(path: pathlib.Path | str = RIBBON) -> dict[str, list[Tab]]:
    """CI の中で使う形。`::error::` を出して 1 で終わる。"""
    try:
        return tables(path)
    except RibbonError as e:
        sys.exit(f"::error::{e}")


# ---------- 自己試験 ----------
#
# **信じる前に、落ちる所を見る。** 6つの崩し方それぞれで止まること。
# `python3 tools/ribbon_parse.py --self-test` で回る(CI からも呼ぶ)。

_BREAKS = {
    "新しい種類 y(…)": lambda s: s.replace('c("open", "開く", "open"),', 'y("open", "開く", "open"),', 1),
    "マクロ書き c!(…)": lambda s: s.replace('c("open", "開く", "open"),', 'c!("open", "開く", "open"),', 1),
    "Tab を関数で作る": lambda s: s.replace('Tab { name: "ファイル", cmds: &[', 'tab("ファイル", &[', 1),
    "c に4つ目の引数": lambda s: s.replace('c("open", "開く", "open"),', 'c("open", "開く", "open", 1),', 1),
    "/* */ で囲んで消す": lambda s: s.replace('c("save", "保存", "save"),', '/* c("save", "保存", "save"), */', 1),
    "表がもう1つ増える": lambda s: s + "\npub const CALC2: &[Tab] = &[];\n",
}


def _self_test() -> int:
    import tempfile

    src = RIBBON.read_text(encoding="utf-8")
    bad = 0

    # まず現物が読めること(残り 0 文字)
    try:
        t = tables()
    except RibbonError as e:
        print(f"::error::現物が読めません: {e}")
        return 1
    for name, tabs in t.items():
        n = sum(len(x.cmds) for x in tabs)
        print(f"  {name}: タブ {len(tabs)} / ボタン {n}(押せる {sum(1 for x in tabs for c in x.cmds if c.ready)})")

    # 崩したら落ちること
    for label, break_it in _BREAKS.items():
        with tempfile.NamedTemporaryFile("w", suffix=".rs", encoding="utf-8", delete=False) as f:
            f.write(break_it(src))
            p = f.name
        try:
            tables(p)
        except RibbonError:
            print(f"  ✓ {label} → 止まった")
        else:
            print(f"::error::{label} を素通りしました(**この検査は効いていません**)")
            bad = 1
        finally:
            pathlib.Path(p).unlink(missing_ok=True)
    return bad


if __name__ == "__main__":
    sys.exit(_self_test() if "--self-test" in sys.argv else (tables_or_die() and 0))
