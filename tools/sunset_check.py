"""**廃止した方針が、現在形の説明として文書に残っていないか。**

2026-08-09 に「ブックはコードを運ばない」が確定した(廃止: `@save` での搭載、
`xl/joPython.xml` への書き込み、ブック由来コードのためのサンドボックス必須)。
ところが README は 2026-08-12 まで9日間、「Python をブックに載せて持ち運べます」
と言い続けていた。マニュアルは同じ回で直っていたのに、README だけ漏れた —
**思い出して直す仕組みしか無かったから**(思い出して叩く台本は検査ではない)。

## どこを見るか

**三帳簿の「使い方」だけ**(README とマニュアル)。設計(SEKKEI.md と
docs/sekkei/)は**見ない** — あちらは「古い節は消さず『↑を改めた』で重ねる」
のが規則で、廃止された言い回しが経緯として残るのが正しい姿だから。
使い方の帳簿は逆で、「できることだけ書く」(docs/README.ja.md 規則3)。

## どう見るか

廃止語の表(下の SUNSET)と突き合わせ、行に廃止語があれば落とす。
ただしその行が**廃止を語っている**(「廃止」「もう無い」「gone」等を含む)
なら通す — 「joPython.xml は廃止した」は正しい記述。

## 限界(SEKKEI「緑は『この物差しでは差が出ない』」の作法)

この検査は**表に載せた語しか見ない**。載せ忘れた廃止は永久に緑で通る。
だから**方針を廃止する回で、この表にも語を足す**こと。それを忘れたら、
この門番自体が「物差しが見ないと決めた所」になる。
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# 使い方の帳簿(glob)。設計(SEKKEI.md / docs/sekkei/)は対象外 — 経緯を残す場所
DOCS = [
    "README.md",
    "README.ja.md",
    "docs/*-manual*.md",
    "docs/from-excel*.md",
    "docs/engine*.md",
    "docs/python-manual*.md",
    "templates/README.md",
    "sample/README.md",
]

# 廃止語の表。(語の正規表現, 廃止を語っている印, いつ・どこで廃止か)
# 印に当たる行は「廃止の説明」なので通す。
SUNSET = [
    (
        re.compile(r"@save"),
        re.compile(r"廃止|もう無い|使えません|gone|removed|no longer|refus"),
        "2026-08-09 廃止(ブック搭載)。正: docs/sekkei/python.ja.md「↑をさらに狭めて」",
    ),
    (
        re.compile(r"ブックに載せ|ブックに搭載|載せて持ち運"),
        re.compile(r"廃止|やめた|もう無い|昔"),
        "2026-08-09 廃止。ブックはコードを運ばない。コードは ~/.config/office/plugins/*.py",
    ),
    (
        re.compile(r"joPython"),
        re.compile(r"廃止|昔の形式|報告|読むだけ|gone|old form|report|read-only"),
        "2026-08-09 廃止(書き込み)。読みは報告つきで残る。正: python.ja.md",
    ),
    (
        re.compile(r"workbook-borne|embeds? code in the workbook|carry Python inside"),
        re.compile(r"gone|removed|no longer|old form|refus"),
        "2026-08-09: workbooks no longer carry code. Code lives in ~/.config/office/plugins/*.py",
    ),
    (
        re.compile(r"ブック由来のコード"),
        re.compile(r"廃止|もう無い|無くなった|来ない"),
        "2026-08-09 廃止。ブック由来のコードという物が存在しなくなった",
    ),
]


def main() -> int:
    files: list[pathlib.Path] = []
    for pat in DOCS:
        files.extend(sorted(ROOT.glob(pat)))
    files = sorted(set(f for f in files if f.is_file()))
    if len(files) < 4:
        # 静かに緑になるのが一番悪い — 対象が読めていないなら落とす
        print(f"::error::対象の文書が {len(files)} 枚しか見つかりません(置き場が変わった?)")
        return 1

    bad = 0
    for f in files:
        rel = f.relative_to(ROOT)
        for i, line in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
            for word, ok, note in SUNSET:
                if word.search(line) and not ok.search(line):
                    print(f"::error file={rel},line={i}::廃止した方針が現在形で残っています: {note}")
                    print(f"  {rel}:{i}: {line.strip()}")
                    bad += 1
    if bad:
        print(f"\n{bad} 件。廃止の経緯として書くなら「廃止」等の語を同じ行に。")
        return 1
    print(f"廃止語の検査: {len(files)} 枚、きれいです。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
