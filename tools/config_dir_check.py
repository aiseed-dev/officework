#!/usr/bin/env python3
"""**設定の置き場の名前**が、実物と食い違っていないか見張る。

正は `pyrun::config_dir()` の `~/.config/officework` と、その下の4つ:

    funcs/    セルから呼ぶ関数(UDF)
    ribbon/   リボンのボタンになるマクロ
    plugins/  一覧から選んで走らせるマクロ
    records/  記録した台本

2026-08-16 に製品名の改名(office → officework)へ置き場を合わせたが、
**利用者が最初に読む所**が古いままだった(2026-08-17 に見つけた):

- リリースの言い分(`.github/workflows/release.yml`)
- `.deb` の説明(`packaging/make-linux.sh`)
- 乗り換えの手引き(`docs/from-excel*.md`)

しかも UDF の置き場は `plugins` から `funcs` に分けたのに、どれも
`plugins` のままだった — **言われたとおりに置いても動かない**案内。
画面が正直でも、文書が嘘なら嘘になる。

    python3 tools/config_dir_check.py
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# 見る所(利用者が読む物と、配る物)
GLOBS = ["docs/*.md", "*.md", "packaging/**/*.sh", "packaging/**/*.md",
         ".github/workflows/*.yml", "ui/*.py", "tools/*.py"]

# 組み立ての残骸は見ない(flatpak-builder が原本を写した物 — 正本ではない)
SKIP = (".flatpak-builder/",)

# 古い名前。`officework` が続くものは正なので外す
STALE = re.compile(r"[.]config[/\\]office(?![\w])")

# この註そのものが経緯を語る所は見逃す(名指しで)
ALLOW = {"tools/config_dir_check.py"}


def main():
    seen = []
    for g in GLOBS:
        for p in sorted(ROOT.glob(g)):
            rel = p.relative_to(ROOT).as_posix()
            if rel in ALLOW or not p.is_file() or any(s in rel for s in SKIP):
                continue
            for n, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
                if STALE.search(line):
                    seen.append((rel, n, line.strip()))
    if seen:
        print(f"**古い置き場の名前が {len(seen)} 箇所あります。**")
        print("正は ~/.config/officework(pyrun::config_dir)。UDF は funcs/ です。\n")
        for rel, n, line in seen:
            print(f"  {rel}:{n}  {line[:90]}")
        return 1
    print("設定の置き場の名前は、どこも ~/.config/officework で揃っています")
    return 0


if __name__ == "__main__":
    sys.exit(main())
