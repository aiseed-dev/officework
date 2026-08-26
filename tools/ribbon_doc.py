#!/usr/bin/env python3
"""操作の一覧を、リボンの表から手引きへ起こす。

    python3 tools/ribbon_doc.py --write   # 手引きの自動生成の節を書き直す
    python3 tools/ribbon_doc.py           # 揃っているか見るだけ(CI の検査)

`keys_doc.py`(キーの一覧)と同じ作法です。書き込む場所は `ribbon:gen` の印の間で、印の外は触りません。

*なぜ要るか。* 2026-08-24 に数えたところ、writer のリボンは 124 ボタンで、
手引きに名前が出るのは 83 個でした。**41 個は、どこを探しても載っていません。**
手引きの本文は「使い方」を書く場所なので全部は並べません。網羅の一覧だけを
この道具が受け持ちます。

手引きの本文にあった「リボンは 117」という数も古くなっていました。
数を手で書くと必ずずれるので、この節では**道具が数えます**。
"""
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).parent))
import ribbon_parse  # noqa: E402

# どの手引きの、どのアプリを載せるか
SAKI = {
    "docs/ja/writer-manual.adoc": ("WRITER", "ja"),
    "docs/en/writer-manual.adoc": ("WRITER", "en"),
    "docs/ja/calc-manual.adoc": ("CALC", "ja"),
    "docs/en/calc-manual.adoc": ("CALC", "en"),
}

MIDASHI = {
    "ja": ("すべての操作", "タブ", "ボタン", "押せるか", "動きます", "まだです"),
    "en": ("All commands", "Tab", "Button", "Ready", "yes", "not yet"),
}


def table(app: str, loc: str) -> str:
    tabs = ribbon_parse.tables_or_die()[app]
    midashi, c1, c2, c3, ok, ng = MIDASHI[loc]
    n = sum(len(t.cmds) for t in tabs)
    ready = sum(1 for t in tabs for c in t.cmds if c.id)
    out = []
    if loc == "ja":
        out.append(f"リボンの全部です。{n} 個のうち {ready} 個が動きます"
                   f"(残り {n - ready} 個はグレー表示です)。\n")
        # **作る側の話は書きません。** 利用者が読む物です
    else:
        out.append(f"All ribbon buttons. {ready} of {n} are wired "
                   f"({n - ready} are greyed out).\n")
    for tab in tabs:
        out.append(f"*{tab.name}*\n")
        out.append('[cols="2,1"]')
        out.append("|===")
        out.append(f"|{c2} |{c3}\n")
        for c in tab.cmds:
            out.append(f"|{c.label} |{ok if c.id else ng}")
        out.append("|===\n")
    return "\n".join(out)


def main() -> int:
    write = "--write" in sys.argv
    bad = 0
    for rel, (app, loc) in SAKI.items():
        p = ROOT / rel
        if not p.exists():
            continue
        src = p.read_text(encoding="utf-8")
        m = re.search(r"(// ribbon:gen:start[^\n]*\n)(.*?)(\n?// ribbon:gen:end)", src, re.S)
        if not m:
            print(f"::error::{rel} に ribbon:gen の印がありません", file=sys.stderr)
            bad = 1
            continue
        ima = m.group(2)
        beki = table(app, loc)
        if ima.strip() == beki.strip():
            continue
        if write:
            p.write_text(src[: m.start(2)] + beki + src[m.end(2):], encoding="utf-8")
            print(f"{rel} を書き直しました")
        elif os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS"):
            # CI では直せません(直しても誰もコミットしない)ので、落として言います
            print(f"::error::{rel} の操作の一覧が実物とずれています"
                  f"(python3 tools/ribbon_doc.py --write で直してコミットしてください)",
                  file=sys.stderr)
            bad = 1
        else:
            # **手元では直します**(2026-08-24 発注者「このような修正で検査が
            # 落ちないようにしろ」)。道具を直して --write を忘れただけなので、
            # 機械が直せます
            p.write_text(src[: m.start(2)] + beki + src[m.end(2):], encoding="utf-8")
            print(f"{rel} がずれていたので直しました。コミットに入れてください")
    if not bad and not write:
        print("操作の一覧は実物と揃っています")
    return bad


if __name__ == "__main__":
    sys.exit(main())
