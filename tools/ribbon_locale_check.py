"""**13言語のリボンが ja と同じ骨組みかを、組み立てずに見る。**

`ui/src/ribbon.rs`(ja)と `ui/src/ribbon_<loc>.rs`(生成物)は、
**語だけが違って id・並び・ready・icon は同じ**でなければならない。
ずれると、日本語では押せるボタンがドイツ語では灰色、といったことが起きる。
`c(…)` が `x(…)` に化けていれば、その言語だけボタンが死ぬ。

同じ照合は `ui/src/ribbon.rs` の `各言語の表は語だけが違う` がしている。
**だが ui は CI で走らない** — `.github/workflows/ci.yml` が gpui の連結を
避けて calc・writer・ui を外しているため。2026-08-10、同じ理由で
`wiring_tests`(押しても何も起きないボタンを止める検査)が CI の外に
あることが分かり、`tools/wiring_check.py` で塞いだ。**これはその一段下**
— 塞いだのは ja の配線だけで、他の12言語の骨組みは誰も見ていなかった。

だからここも**原文の表を読む**。`ribbon.rs` は gpui を1度も使っておらず、
生成物も素のリテラルなので、コンパイラは要らない。

**この検査は、見えなくなったときに落ちる。** 字面を読む検査の危険は
「書き方が変わって何も拾えなくなり、静かに緑になる」ことで、それは
この検査が止めたい欠陥と同じ形。だから拾えた数が少なすぎたら落とす。
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
UI = ROOT / "ui/src"

# 拾えた数がこれを下回ったら「読めていない」と見なす。
# いまは CALC・WRITER とも 200 前後なので、半分を切ったら書き方が変わった
FLOOR = 80


def buttons(path: pathlib.Path, table: str) -> list[tuple[str, str, bool]]:
    """`pub const CALC: &[Tab] = &[ … ];` から (id, icon, ready) を順に拾う。

    `c("id", "語", "icon")` が押せるボタン、`x("語")` が灰色。
    灰色は id を持たないので、**位置を保つために印だけ積む** —
    数だけ合っていて並びがずれる、を見逃さないため。
    """
    src = path.read_text(encoding="utf-8")
    m = re.search(rf"pub const {table}: &\[Tab\] = &\[(.*?)^\];", src, re.S | re.M)
    if not m:
        sys.exit(f"::error::{path.name} の {table} の表が見つかりません(書き方が変わった?)")
    body = re.sub(r"//[^\n]*", "", m.group(1))
    out: list[tuple[str, str, bool]] = []
    for kind, args in re.findall(r"\b([cx])\(\s*((?:[^()]|\([^()]*\))*)\)", body):
        lits = re.findall(r'"((?:[^"\\]|\\.)*)"', args)
        if kind == "c" and len(lits) >= 3:
            out.append((lits[0], lits[2], True))
        elif kind == "x":
            out.append(("", "", False))
    return out


def tabs(path: pathlib.Path, table: str) -> list[str]:
    """タブの並び(名前は訳されるので**数だけ**見る)。"""
    src = path.read_text(encoding="utf-8")
    m = re.search(rf"pub const {table}: &\[Tab\] = &\[(.*?)^\];", src, re.S | re.M)
    return re.findall(r"\bTab\s*\{", m.group(1)) if m else []


def main() -> int:
    locales = sorted(
        p.stem[len("ribbon_"):]
        for p in UI.glob("ribbon_*.rs")
        if p.stem not in ("ribbon_tables",)
    )
    if not locales:
        print("::error::ui/src に ribbon_<loc>.rs がありません")
        return 1

    bad = 0
    for table in ("CALC", "WRITER"):
        ja = buttons(UI / "ribbon.rs", table)
        if len(ja) < FLOOR:
            print(f"::error::{table}: ja の表が読めていません(ボタン {len(ja)} 件)")
            bad = 1
            continue
        ja_tabs = len(tabs(UI / "ribbon.rs", table))
        for loc in locales:
            got = buttons(UI / f"ribbon_{loc}.rs", table)
            if len(got) != len(ja):
                print(
                    f"::error::{table} {loc}: ボタンの数が違います"
                    f"(ja {len(ja)} / {loc} {len(got)})"
                )
                bad = 1
                continue
            n = len(tabs(UI / f"ribbon_{loc}.rs", table))
            if n != ja_tabs:
                print(f"::error::{table} {loc}: タブの数が違います(ja {ja_tabs} / {loc} {n})")
                bad = 1
            for i, (a, b) in enumerate(zip(ja, got)):
                if a != b:
                    print(
                        f"::error::{table} {loc}: {i} 番目のボタンがずれています "
                        f"— ja (id={a[0]!r} icon={a[1]!r} ready={a[2]}) / "
                        f"{loc} (id={b[0]!r} icon={b[1]!r} ready={b[2]})"
                    )
                    bad = 1
                    break
        if not bad:
            print(f"{table}: {len(locales)} 言語とも ja と同じ骨組み(ボタン {len(ja)} 件)")
    return bad


if __name__ == "__main__":
    raise SystemExit(main())
