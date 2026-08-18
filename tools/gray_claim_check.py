"""**文書が実物より多く名乗っていないか。**

家訓は「できないものを、できるように見せない」。画面はそれを守っていた —
未実装のボタンはちゃんと灰色で並ぶ。**破っていたのは文書のほう**で、
README と手引きの3箇所が「グレー表示ゼロ」と言い続けていた。実際は calc に
9個ある(2026-08-10 に数えて発覚)。

読む人は文書を先に読む。**画面が正直でも、文書が嘘なら嘘をついたことになる。**

数え方も3つ混ざっていた(155/155・145/145・実測はボタン 204 個)。だから
ここでは**リボンの表を数えた値だけ**を正とし、文書がその数を書いているかを見る。

## どう見るか

灰色について**数を主張している行**を拾い、その行に**本当の数が書いてあるか**を
見る。「ゼロ」「zero」「no」は 0 と読む。数を言っていない行(「未実装のものは
グレー表示で並んでいます」)は主張ではないので通す。

だから数が変わったのに文書を直さなければ落ちる ── **この検査が止めたいのは、
まさにそれ**。
"""

from __future__ import annotations

import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import ribbon_parse  # noqa: E402  (表を読むのはここだけ)

ROOT = pathlib.Path(__file__).resolve().parent.parent

# 灰色は `x("名前", "図")` で書く(`c(…)` が押せるほう)
GRAY = re.compile(r'\bx\(\s*"')
# 拾えた数がこれを下回ったら「表を読めていない」。静かに緑になるのが一番悪い
FLOOR = 60

# 文書 → その文書が語っているアプリ
DOCS = {
    "README.adoc": ("writer", "calc"),
    "README.ja.adoc": ("writer", "calc"),
    "docs/calc-manual.adoc": ("calc",),
    "docs/calc-manual.ja.adoc": ("calc",),
    "docs/writer-manual.adoc": ("writer",),
    "docs/writer-manual.ja.adoc": ("writer",),
}

# 灰色の話をしている行か
MENTIONS = re.compile(r"グレー|gray|grey", re.I)
# **これから入る分の印。** 手引きが次の版を先に説明するときに付ける
# (例: 「リボンは 117 — 全部のボタンが動く(0.2.0-alpha で入ります)」)
MADA = re.compile(r"で入ります|coming in ")

# その行が数を主張しているか(「ゼロ」「zero」「no grayed」も 0 の主張)
ZERO_WORDS = re.compile(r"ゼロ|\bzero\b|\bno\b", re.I)


def gray_counts() -> dict[str, int]:
    """表ごとに灰色のボタンを数える。

    **床(`FLOOR`)は要らなくなった**(2026-08-12)。前は正規表現で拾う形
    だったので「拾えなくなって 0 になる」を床で防いでいたが、いまは
    `ribbon_parse` が**食べ尽くして残りが出たら落ちる** — 読めなくなったら
    数える前に止まる。床は「全盲」しか拾えず、半分になった状態は
    素通りしていた。
    """
    t = ribbon_parse.tables_or_die()
    return {app: sum(1 for tab in t[table] for c in tab.cmds if not c.ready)
            for table, app in (("WRITER", "writer"), ("CALC", "calc"))}


def main() -> int:
    gray = gray_counts()
    print("実物の灰色: " + " / ".join(f"{a} {n} 個" for a, n in gray.items()))
    bad = 0
    for path, apps in DOCS.items():
        f = ROOT / path
        if not f.exists():
            continue
        want = {gray[a] for a in apps}
        for i, line in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
            if not MENTIONS.search(line):
                continue
            # **これから入る分の印がある行は、いまの主張ではない。**
            # 手引きは「次の版の説明」を先に書くことがある(2026-08-18 発注者
            # 「バージョンを決めて手引きを先に全部直せというのは、悪くはない
            # のでは」)。印のある行は、その版が出るまで数を見ない
            if MADA.search(line):
                continue
            # **灰色という語のすぐ近くの数だけを見る。** 行に数があれば
            # 全部主張だと見なすと、「All 114 ribbon commands work. The gray
            # of …」のような別の話の数まで拾う(最初の版がこれで誤検知した)
            said: set[int] = set()
            for m in MENTIONS.finditer(line):
                # 前は狭く、後ろは広く。英語は数が前に来て(zero grayed out /
                # 9 grayed-out)、日本語は後ろに来る(グレー表示は 9 個)
                w = line[max(0, m.start() - 14) : m.end() + 40]
                said |= {int(n) for n in re.findall(r"\d+", w)}
                if ZERO_WORDS.search(w):
                    said.add(0)
            if not said:
                continue  # 数を言っていない行は主張ではない
            missing = want - said
            if missing:
                print(f"::error::{path}:{i}: 灰色は {sorted(want)} 個なのに、"
                      f"この行は {sorted(said)} と言っています")
                print(f"    {line.strip()[:120]}")
                bad = 1
    if not bad:
        print("文書の主張は実物と合っています")
    return bad


if __name__ == "__main__":
    raise SystemExit(main())
