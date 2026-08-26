#!/usr/bin/env python3
"""**状態行の文言が訳を通っているか**(2026-08-21)。

画面の下に出る1行(`self.status`)は `ui::t!` / `ui::tf!` で書きます。
そうすると対訳の表に載り、14 言語で各国語が出ます。

ところが `format!` で書いても**コンパイルは通ります**。書いた人の画面
(日本語)では正しく見えるので、そのまま気づかれません。**その言語で
使う人以外は誰も気づかない**という、この一連の欠陥と同じ形です。

## 実際にあった数

2026-08-21 に数えたら、**日本語を含む `format!` の状態行が 26 件**
ありました(表 25・文章 1)。どれも 14 言語で日本語がそのまま出ます。
設計が `ai-where` の例で挙げたのと同じ欠陥が、これだけ残っていました。

## 拾わないもの

穴と英数字だけの物は訳が要りません(`"{}:{}"`、`"AI: {e}"` など)。
だから**日本語の字が入っているかどうか**で見ます。

## 直し方

`format!` を `ui::tf!` に替え、名前つきの差し込み(`{name}`)は位置
(`{}`)に直します。訳す人が名前を保つ必要がなくなるためです。
そのあと `ui/gen_i18n.py --missing` から始まる i18n の手順を通します。

## 使い方

    python3 tools/status_yaku_check.py
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
check = ("calc/src", "writer/src", "ui/src", "officework/src")

# `self.status = format!(` / `this.status =\n format!(` の両方を拾う
shape = re.compile(r'status\s*=\s*\n?\s*format!\(\s*\n?\s*(".*?")', re.S)
日本語 = re.compile(r'[ぁ-んァ-ヶ一-龠]')


def main() -> int:
    seen, bad = 0, []
    for d in check:
        for p in sorted((ROOT / d).rglob("*.rs")):
            s = p.read_text(encoding="utf-8")
            for m in shape.finditer(s):
                seen += 1
                sentence = m.group(1)[:160]
                if 日本語.search(sentence):
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
