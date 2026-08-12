"""**押しても何も起きないボタンが main に入るのを止める。**

リボンで `ready`(押せる見た目)にしてあるコマンドが、アプリの `HANDLED` に
載っているかを見る。載っていなければ、画面では押せるのに何も起きない。

同じ照合は `calc/src/tests.rs` と `writer/src/tests.rs` の `wiring_tests` が
している。**2026-08-10 の時点では、それが CI で一度も走っていなかった** —
gpui の連結が要るため `ci.yml` が意図して外していて、つまり「ready の嘘は
wiring_tests が落とす」という方針は**手元で誰かが cargo test を打ったときだけ**
成り立っていた(fork セッションが CI の対象を数えて見つけた)。

**その前提は `d1d120b`(2026-08-11)で失効した。** いま ci.yml には
「cargo test(画面のいる calc / writer)」の仕事があり、`wiring_tests` は
CI で走る。ここはもう**唯一の防波堤ではなく、速いほうの防波堤**:

- gpui の要らない安い仕事で、数秒で落ちる
- 画面のいる仕事は GitHub のランナーで**まだ日が浅い**

**畳むのは「画面のいる仕事が何日か続けて緑」を見てから。** 忘れると、
重複した検査を永久に二重で保守することになる(期限つきの宿題)。

読み方は**組み立てずに原文の表そのもの**。`ui/src/ribbon.rs` は gpui を
1度も使っていない素のリテラルなので、これで足りる。

**表を読むのは `ribbon_parse` に集めてある**(2026-08-12)。5つの道具が
それぞれ正規表現で読んでいて、**全部が「拾う」形=拾えなかった物を黙って
捨てる**形だった。いまは食べ尽くす形で、知らない書き方が1つでもあれば
読む前に落ちる。
"""

from __future__ import annotations

import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import ribbon_parse  # noqa: E402  (同じ棚の1枚。表を読むのはここだけ)

ROOT = pathlib.Path(__file__).resolve().parent.parent

# 拾えた数がこれを下回ったら「読めていない」と見なす。**実際に数えた値**は
# CALC 195・WRITER 126(2026-08-10)。README の「145/124」は別の数え方
# (ボタンではなくコマンド)なので、床の目安には使わない
FLOOR = 60


def ready_ids(table: str) -> list[str]:
    """押せるボタンの id を順に。灰色(`x`)は id を持たないので出てこない。"""
    return [c.id for tab in ribbon_parse.tables_or_die()[table] for c in tab.cmds if c.ready]


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
