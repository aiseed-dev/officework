#!/usr/bin/env python3
"""AsciiDoc の書き方の表が、本家(asciidoctor)とずれていないか見る。

writer は AsciiDoc の視覚エディタです。**編集できるのは部分集合でよいが、
表示はすべてできないといけない**(発注者 2026-08-18)。そのためには「本家に
どんな書き方があるか」を知っている必要があり、それを**記憶で書かない**ための
道具です。

正本は本家の `lib/asciidoctor.rb` の `DELIMITED_BLOCKS` などです。写しを
`docs/sekkei/asciidoctor-syntax.json` に置き、engine の表と突き合わせます。

    python3 tools/adoc_syntax_check.py            # 突き合わせる
    python3 tools/adoc_syntax_check.py --update   # 本家から写しを取り直す

`--update` には `vendor/asciidoctor`(追跡していません)が要ります:

    git clone --depth 1 https://github.com/asciidoctor/asciidoctor.git vendor/asciidoctor
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
copy = ROOT / "docs/sekkei/asciidoctor-syntax.json"
vendor = ROOT / "vendor/asciidoctor/lib/asciidoctor.rb"
ADOC_RS = ROOT / "engine/src/adoc.rs"

# うちが**意味を知っていて編集もできる**区切り。表に無くてよい
editable = {"____", "|==="}


def read_from_vendor():
    src = vendor.read_text(encoding="utf-8")
    m = re.search(r"DELIMITED_BLOCKS = \{(.*?)\n  \}", src, re.S)
    blocks = dict(re.findall(r"'([^']+)' => \[:(\w+)", m.group(1)))
    adm = re.findall(
        r"'([A-Z]+)'",
        re.search(r"ADMONITION_STYLES = ::Set\[([^\]]+)\]", src).group(1),
    )
    lay = dict(
        re.findall(
            r"'(\\?.)' => :(\w+),",
            re.search(r"LAYOUT_BREAK_CHARS = \{(.*?)\}", src, re.S).group(1),
        )
    )
    md = dict(
        re.findall(
            r"'(.)' => :(\w+),",
            re.search(r"MARKDOWN_THEMATIC_BREAK_CHARS = \{(.*?)\}", src, re.S).group(1),
        )
    )
    return {
        "delimited_blocks": blocks,
        "admonitions": adm,
        "layout_breaks": lay,
        "markdown_breaks": md,
    }


def engine_no_hyou():
    src = ADOC_RS.read_text(encoding="utf-8")
    m = re.search(r"const DELIMITED: &\[\(&str, &str\)\] = &\[(.*?)\n\];", src, re.S)
    if not m:
        sys.exit("engine/src/adoc.rs の DELIMITED が読めません(表の形が変わった?)")
    mark = set(re.findall(r'\("([^"]+)",', m.group(1)))
    a = re.search(r"const ADMONITION: &\[&str\] = &\[(.*?)\];", src, re.S)
    if not a:
        sys.exit("engine/src/adoc.rs の ADMONITION が読めません")
    admon = {x.rstrip(":") for x in re.findall(r'"([^"]+)"', a.group(1))}
    return mark, admon


def main():
    if "--update" in sys.argv:
        if not vendor.is_file():
            sys.exit(f"{vendor} がありません。README の手順で vendor/asciidoctor を置いてください")
        older = json.loads(copy.read_text(encoding="utf-8")) if copy.is_file() else {}
        fresh = {k: v for k, v in older.items() if k.startswith("_")}
        fresh.update(read_from_vendor())
        copy.write_text(
            json.dumps(fresh, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
        )
        print(f"{copy} を取り直しました")
        return

    if not copy.is_file():
        sys.exit(f"{copy} がありません(--update で作ります)")
    table = json.loads(copy.read_text(encoding="utf-8"))

    # 本家が手元にあるなら、写しが古くないかも見る(`_` で始まる鍵は覚え書き)
    content = {k: v for k, v in table.items() if not k.startswith("_")}
    if vendor.is_file() and read_from_vendor() != content:
        sys.exit(f"{copy} が本家より古いです(--update で取り直してください)")
    table = content

    mark, admon = engine_no_hyou()
    bad = []
    for k in table["delimited_blocks"]:
        if k not in editable and k not in mark:
            bad.append(f"塊の区切り「{k}」({table['delimited_blocks'][k]})を engine が知りません")
    for a in table["admonitions"]:
        if a not in admon:
            bad.append(f"註記「{a}」を engine が知りません")
    if bad:
        print("本家の AsciiDoc にあって、engine が知らない書き方があります:")
        for x in bad:
            print("  -", x)
        print("engine/src/adoc.rs の DELIMITED / ADMONITION に足してください。")
        sys.exit(1)
    n = len(table["delimited_blocks"]) + len(table["admonitions"])
    print(f"本家の書き方 {n} 種と揃っています(区切り {len(table['delimited_blocks'])}・"
          f"註記 {len(table['admonitions'])})")


if __name__ == "__main__":
    main()
