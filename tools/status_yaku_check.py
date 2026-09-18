#!/usr/bin/env python3
"""Check that status bar text goes through translation (2026-08-21).

The single line shown at the bottom of the screen (`self.status`) is written
with `ui::t!` or `ui::tf!`. That puts it in the translation table, so it appears
in each of the 14 languages.

Writing it with `format!` compiles just as well. On the author's own screen (in
Japanese) it looks right, so nobody notices. It is the same shape of defect as
the rest of this series: nobody except a user of that language ever notices.

## The count we actually found

Counting on 2026-08-21 there were 26 status bar strings written with `format!`
that contained Japanese (25 in tables and 1 in prose). All of them show Japanese
as is in all 14 languages. That many instances of the defect the design document
gives as the `ai-where` example were still left.

## What is not reported

A string made only of placeholders and ASCII needs no translation (`"{}:{}"`,
`"AI: {e}"` and the like). So the test looks at whether the string contains any
Japanese characters.

## How to fix it

Replace `format!` with `ui::tf!`, and change named placeholders (`{name}`) to
positional ones (`{}`), so that translators no longer have to keep the names.
Then go through the i18n steps starting with `ui/gen_i18n.py --missing`.

## Usage

    python3 tools/status_yaku_check.py
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
check = ("calc/src", "writer/src", "ui/src", "officework/src")

# `self.status = format!(` / `this.status =\n format!(` の両方を拾う
shape = re.compile(r'status\s*=\s*\n?\s*format!\(\s*\n?\s*(".*?")', re.S)
japanese = re.compile(r'[ぁ-んァ-ヶ一-龠]')


def main() -> int:
    seen, bad = 0, []
    for d in check:
        for p in sorted((ROOT / d).rglob("*.rs")):
            s = p.read_text(encoding="utf-8")
            for m in shape.finditer(s):
                seen += 1
                sentence = m.group(1)[:160]
                if japanese.search(sentence):
                    line = s[: m.start()].count("\n") + 1
                    bad.append((p.relative_to(ROOT), line, sentence[:70]))
    # **読めなくなったら落ちる。** 静かに緑になるのが一番悪い。
    # 穴と英数字だけの `format!` は今も 10 件ほどあるので、0 は「読めていない」
    if seen < 5:
        print(f"::error::status = format! が {seen} 件しか見つかりません(書き方が変わった?)")
        return 1
    for f, line, sentence in bad:
        print(
            f"::error::{f}:{line} 状態行が訳を通っていません — {sentence}。"
            "format! ではなく ui::tf! で書いてください"
            "(名前つきの差し込みは位置に直す)"
        )
    if bad:
        return 1
    print(f"状態行の format! {seen} 件は、どれも訳の要らない物です(日本語を含みません)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
