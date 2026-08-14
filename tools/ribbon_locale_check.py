"""**13言語のリボンが ja と同じ骨組みかを、組み立てずに見る。**

`face/src/ribbon.rs`(ja)と `face/src/ribbon_<loc>.rs`(生成物)は、
**語だけが違って id・並び・ready・icon は同じ**でなければならない。
ずれると、日本語では押せるボタンがドイツ語では灰色、といったことが起きる。
`c(…)` が `x(…)` に化けていれば、その言語だけボタンが死ぬ。

同じ照合は `face/src/ribbon.rs` の `各言語の表は語だけが違う` がしている。
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

import functools
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import ribbon_parse  # noqa: E402  (表を読むのはここだけ)

ROOT = pathlib.Path(__file__).resolve().parent.parent
# リボンの表と14言語は face(gpui を持たない層)へ移った(2026-08-15)
UI = ROOT / "face/src"

# 拾えた数がこれを下回ったら「読めていない」と見なす。
# いまは CALC・WRITER とも 200 前後なので、半分を切ったら書き方が変わった
FLOOR = 80


@functools.lru_cache(maxsize=None)
def _tables(path: pathlib.Path):
    """**表を読むのは `ribbon_parse` だけ。**

    ここには3つの読み手(骨組み・タブ・語)があり、**3つとも別々の正規表現で
    同じ物を拾っていた**(2026-08-12 に統合)。拾う形は拾えなかった物を黙って
    捨てるので、書き方が変われば3つとも静かに減る。いまは食べ尽くす形で、
    知らない書き方が1つでもあれば読む前に落ちる。
    """
    return ribbon_parse.tables_or_die(path)


def buttons(path: pathlib.Path, table: str) -> list[tuple[str, str, bool]]:
    """(id, icon, ready) を順に。

    **灰色も印だけ積む** — 数だけ合っていて並びがずれる、を見逃さないため。
    """
    return [
        (c.id, c.icon, True) if c.ready else ("", "", False)
        for tab in _tables(path)[table]
        for c in tab.cmds
    ]


def tabs(path: pathlib.Path, table: str) -> list[str]:
    """タブの並び。**名前は訳されるので数だけ**見る。"""
    return [tab.name for tab in _tables(path)[table]]


def labels(path: pathlib.Path, table: str) -> list[str]:
    """ボタンに**出る語**を順に。骨組みではなく中身を見るときに使う。"""
    return [c.label for tab in _tables(path)[table] for c in tab.cmds]


def same_words(locales: list[str]) -> int:
    """**語まで丸ごと同じ2言語が無いこと。**

    2026-08-11、欧州ポルトガル語の札を `pt-PT` から `pt` に変えたら、
    生成器が本家の `pt.json`(こちらの綴りと逆で**ブラジル語**)を
    黙って拾い、欧州版のリボンがブラジル語で出来上がっていた。
    骨組みの検査は id と並びしか見ないので、**中身が別言語でも緑**だった。

    2つの言語が1文字も違わないなら、まず同じ材料から作っている。

    **この検査だけでは足りない。** 当時は pt-BR がまだ無く、比べる相手が
    いなかったので、これでも見つけられなかった。実際に効いたのは
    「本家の欧州ファイルは薄いので、読み替えれば訳の欠けが露見する」
    ほうだった。二重に見る。
    """
    bad = 0
    for table in ("CALC", "WRITER"):
        seen: dict[tuple[str, ...], str] = {}
        for loc in locales:
            key = tuple(labels(UI / f"ribbon_{loc}.rs", table))
            if key in seen:
                print(
                    f"::error::{table}: {seen[key]} と {loc} の語が完全に同じです。"
                    "同じ材料から作っていませんか"
                    "(gen_ribbon_locale.py の VENDOR_LOCALE を確かめてください)"
                )
                bad = 1
            seen[key] = loc
    return bad


def main() -> int:
    locales = sorted(
        p.stem[len("ribbon_"):]
        for p in UI.glob("ribbon_*.rs")
        if p.stem not in ("ribbon_tables",)
    )
    if not locales:
        print("::error::face/src に ribbon_<loc>.rs がありません")
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
    bad |= same_words(locales)
    if not bad:
        print(f"語の重なり: {len(locales)} 言語とも別の語で出来ています")
    return bad


if __name__ == "__main__":
    raise SystemExit(main())
