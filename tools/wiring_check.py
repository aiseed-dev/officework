"""**押しても何も起きないボタンが main に入るのを止める。**

リボンで `ready`(押せる見た目)にしてあるコマンドが、アプリの `HANDLED` に
載っているかを見る。載っていなければ、画面では押せるのに何も起きない。

同じ照合は `calc/src/tests.rs` と `writer/src/tests.rs` の `wiring_tests` が
していた。**だが calc と writer は CI で一度も走っていない**(gpui の連結が
要るため、`.github/workflows/ci.yml` が意図して外している)。つまり
「ready の嘘は wiring_tests が落とす」という方針は、**手元で誰かが
cargo test を打ったときだけ成り立っていた**(2026-08-10、fork セッションが
CI の対象を数えて見つけた)。

だからここでは**組み立てずに、原文の表そのものを読む**。どちらの表も素の
リテラルで、`ui/src/ribbon.rs` は gpui を1度も使っていない。

**この検査は、見えなくなったときに落ちる。** 字面を読む検査の一番の危険は
「書き方が変わって何も拾えなくなり、静かに緑になる」ことで、それは
まさに今日見つけた欠陥そのものの形だから、拾えた数が少なすぎたら落とす。
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# 拾えた数がこれを下回ったら「読めていない」と見なす。**実際に数えた値**は
# CALC 195・WRITER 126(2026-08-10)。README の「145/124」は別の数え方
# (ボタンではなくコマンド)なので、床の目安には使わない
FLOOR = 60


def ready_ids(table: str) -> list[str]:
    """`ui/src/ribbon.rs` の `pub const CALC: &[Tab] = &[ … ];` から
    `c("id", …)`(= ready: true)の id を順に拾う。

    `x(…)`(灰色)は id を持たないので、そもそも拾えない。
    """
    src = (ROOT / "ui/src/ribbon.rs").read_text(encoding="utf-8")
    m = re.search(rf"pub const {table}: &\[Tab\] = &\[(.*?)^\];", src, re.S | re.M)
    if not m:
        sys.exit(f"::error::ui/src/ribbon.rs の {table} の表が見つかりません(書き方が変わった?)")
    return re.findall(r'\bc\(\s*"([^"]+)"', m.group(1))


def handled(crate: str) -> set[str]:
    """`const HANDLED: &'static [&'static str] = &[ … ];` の中の文字列を拾う。

    **ファイルを名指ししない。** `<crate>/src` を舐めて探す — 2026-08-10 の
    部屋割りで `writer` の `HANDLED` が `main.rs` から `keys.rs` へ移り、
    名指しだったこの検査が落ちた。落ちたこと自体は設計どおり(静かに緑に
    なるよりずっと良い)だが、**割るたびに検査を追いかけるのは筋が悪い**。
    文言の門番と同じく、置き場を舐める形に揃える。
    """
    hits = []
    for f in sorted((ROOT / crate / "src").glob("*.rs")):
        m = re.search(
            r"const HANDLED: &'static \[&'static str\] = &\[(.*?)^\s*\];",
            f.read_text(encoding="utf-8"),
            re.S | re.M,
        )
        if m:
            hits.append((f.name, m.group(1)))
    if not hits:
        sys.exit(f"::error::{crate}/src に HANDLED がありません(書き方が変わった?)")
    if len(hits) > 1:
        # 2つあったら、どちらが本物か決められない — 黙って片方を採らない
        sys.exit(
            f"::error::{crate}/src に HANDLED が {len(hits)} 個あります: "
            + ", ".join(n for n, _ in hits)
        )
    # 行末の注釈に文字列が入ることがあるので、コメントを落としてから拾う
    body = re.sub(r"//[^\n]*", "", hits[0][1])
    return set(re.findall(r'"([^"]*)"', body))


def main() -> int:
    bad = 0
    for table, app in (("CALC", "calc"), ("WRITER", "writer")):
        ready = ready_ids(table)
        known = handled(app)
        # **読めなくなったら落ちる。** 静かに緑になるのが一番悪い
        if len(ready) < FLOOR or len(known) < FLOOR:
            print(f"::error::{app}: 表が読めていません(ready {len(ready)} / handled {len(known)})")
            bad = 1
            continue
        missing = [i for i in ready if i and i not in known]
        if missing:
            print(f"::error::{app}: 押せる見た目なのに {app} が知らないコマンド: {', '.join(missing)}")
            bad = 1
        else:
            print(f"{app}: ready {len(ready)} 件、全部配線されています")
    return bad


if __name__ == "__main__":
    raise SystemExit(main())
