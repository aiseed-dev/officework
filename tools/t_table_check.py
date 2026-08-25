#!/usr/bin/env python3
"""**画面の文言が対訳の表を通っているか**を見張る。

`ui::t!("…")` / `ui::tf!("…")` に書いた日本語が対訳の材料
(`ui/i18n/keys.json` — i18n_en.rs の表 + リボンの札)に無ければ、
その句は**13言語すべてで日本語のまま出る**。落ちるのは画面ではなく
翻訳なので、試験も画面も何も言わない。

2026-08-15、左右のパネルを入れたときに 48 句が黙って素通りした。
「嘘は欠落より悪い」— 英語の画面に日本語が1行混じるのは、無いより悪い。
そこでこの門番を置く。

    python3 tools/t_table_check.py

手順(memory の「i18n の手順」と同じ):

1. `lang/src/i18n_en.rs` に (日本語, English) を足す
2. `python3 ui/gen_lang.py --todo`(keys.json を作り直す)
3. `python3 tools/i18n_remap.py --old <前の keys.json>`
4. `ui/i18n/<loc>.json` に訳を足し、`python3 ui/gen_lang.py <loc>` で13言語
"""

import json
import os
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "ui"))
import gen_lang as g  # noqa: E402

# 見る場所。**試験は見ない**(試験の中の日本語は画面に出ない)
# **Rust 側の門番(lang/tests/i18n_soroi.rs)と同じ場所を見る。**
# lang/src は入れない — 対訳の仕組みそのものが `t!("…")` を使っていて、
# あれは画面の文言ではない。face/src は足してある(表の側に文言が
# 増えても拾えるように。いまは空)
DIRS = ("calc/src", "writer/src", "ui/src", "face/src")
SKIP = {"tests.rs"}
MACROS = ("ui::t!(", "ui::tf!(", "t!(", "tf!(")


def literals(src):
    """ソースから t!/tf! の第1引数の文字列リテラルを拾う。

    **1行の正規表現では足りない。** 行継続(`\\` 改行)で折った長い案内は
    素通りし、実際に4句が漏れた(2026-08-15)。リテラルの終わりは
    gen_lang の読み手に任せる。
    """
    out = []
    for macro in MACROS:
        i = 0
        while True:
            i = src.find(macro, i)
            if i < 0:
                break
            # `ui::t!` の中の `t!` を二度数えない
            if macro in ("t!(", "tf!(") and i > 0 and src[i - 1] in ":_a-zA-Z":
                i += len(macro)
                continue
            j = i + len(macro)
            while j < len(src) and src[j] in " \n\t":
                j += 1
            if j < len(src) and src[j] == '"':
                _, lit = g.literal_at(src, j)
                out.append(g.unescape(lit))
            i = j
    return out


def main():
    keys = ROOT / "ui/i18n/keys.json"
    if not keys.exists():
        print("ui/i18n/keys.json がありません(python3 ui/gen_lang.py --todo で作る)")
        return 1
    # **鍵は英語です**(2026-08-26)。ソースの字と鍵の正本を直に比べます
    ja = {e["key"] for e in json.load(open(keys, encoding="utf-8"))}
    miss = {}
    for d in DIRS:
        for p in sorted((ROOT / d).glob("*.rs")):
            if p.name in SKIP:
                continue
            for s in literals(p.read_text(encoding="utf-8")):
                # **日本語を含まない句も飛ばさない。** 飛ばしていたら
                # `{:.0}×{:.0}mm` が素通りし、Rust 側の門番
                # (lang/tests/i18n_soroi.rs)だけが落ちた(2026-08-15)。
                # **門番が2つで食い違うのは、1つも無いより悪い**
                if s not in ja:
                    miss.setdefault(s, set()).add(os.path.join(d, p.name))
    if not miss:
        print(f"文言は全部 対訳の表を通っています(材料 {len(ja)} 句)")
        return 0
    print(f"**対訳の表に無い文言が {len(miss)} 句あります。**")
    print("このままだと13言語すべてで日本語のまま出ます。")
    print("python3 ui/gen_lang.py --todo で鍵の正本に足し、各言語の訳を"
          "書いてから生成し直してください。\n")
    for s, files in sorted(miss.items()):
        head = s if len(s) <= 46 else s[:46] + "…"
        print(f"  {head}\n      {' '.join(sorted(files))}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
