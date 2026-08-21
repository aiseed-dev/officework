#!/usr/bin/env python3
"""**同じ日本語には同じ訳**(2026-08-21)。

officework の訳の材料は `ui/i18n/keys.json` で、番号がついています。
ところが**同じ日本語が2つの番号を持つ**ことがあります。材料が2つの
出どころから来ているためです。

* 文言 — `lang/src/i18n_en.rs`(`ui::t!` や `ui::item!` で使う語)
* リボンの語 — `ui/gen_ribbon_locale.py` の `OVERRIDES`(本家に無いボタンの語)

日本語では1つの語なのに、番号が別なので**訳を別々に書けてしまいます**。
実際、2026-08-21 に「セルの書式設定」がリボンと保護の設定の一覧で
別の語になっている言語が8つありました(スペイン語なら
`Dar formato a celdas` と `Aplicar formato a celdas`)。
利用者から見れば同じ機能の名前なので、揃っていないと探せません。

## 直し方

どちらの語を使うかは、**本家に載っている方**を採ります
(2026-08-21 の決め「訳は本家から取る」)。両方載っている・
どちらも載っていないときは人が決めます。決めた語を
`ui/i18n/<言語>.json` の**両方の番号**に書いてください。

## まだ揃っていない組

`未決` に書きます。**いまは空です**(2026-08-21 に 17 件を片付けました)。

決め方は2段です。

1. **本家に載っている方**を採る(2026-08-21 の決め「訳は本家から取る」)
2. 本家で決まらないときは**リボンの語**を採る — 利用者がボタンで読む
   名前なので、案内の文もその名前で呼ぶのが筋です

例外は、リボンの語が明らかに誤りのときです。ドイツ語の「スタイル」は
`Typ`(種類)でした。略語(`Math. u. Trigonom.`)も、吹き出しに出る語で
幅の制約が無いので、全部書く形に寄せました。

## 使い方

    python3 tools/i18n_onaji_go_check.py
"""
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "ui"))
import locales  # noqa: E402

# **本家では決まらなかった組**(2026-08-21)。どちらの語も本家に載って
# いる、または両方とも載っていないので、機械では選べません。
#
# 鍵は日本語、値はまだ食い違っている言語です。決まったら消してください。
未決: dict[str, set[str]] = {}


def main() -> int:
    keys = json.loads((ROOT / "ui/i18n/keys.json").read_text(encoding="utf-8"))
    組: dict[str, list[int]] = {}
    for k in keys:
        組.setdefault(k["ja"], []).append(k["i"])
    重 = {w: v for w, v in 組.items() if len(v) > 1}
    if len(重) < 5:
        # **読めなくなったら落ちる。** 静かに緑になるのが一番悪い
        print(f"::error::同じ原文の組が {len(重)} 件しかありません(材料の形が変わった?)")
        return 1

    locs = [t for t in locales.TAGS if t != "en"]
    訳 = {}
    for loc in locs:
        p = ROOT / "ui/i18n" / f"{loc}.json"
        訳[loc] = {
            x["i"]: x.get("t")
            for x in json.loads(p.read_text(encoding="utf-8"))
            if isinstance(x, dict)
        }

    bad = 0
    見た: dict[str, set[str]] = {}
    for w, idx in sorted(重.items()):
        for loc in locs:
            v = {訳[loc].get(i) for i in idx if 訳[loc].get(i)}
            if len(v) <= 1:
                continue
            見た.setdefault(w, set()).add(loc)
            if loc in 未決.get(w, set()):
                continue
            print(
                f"::error::{loc}: {w!r} の訳が {sorted(v)} と分かれています"
                f"(番号 {idx})。本家に載っている方に揃えるか、"
                "tools/i18n_onaji_go_check.py の 未決 に足してください"
            )
            bad = 1

    # 直したのに 未決 に残っていると、次の食い違いを見逃します
    for w, ls in 未決.items():
        余り = ls - 見た.get(w, set())
        if 余り:
            print(
                f"::error::{w!r} は {sorted(余り)} で揃っているのに 未決 に残っています。"
                "表から消してください"
            )
            bad = 1

    if not bad:
        の数 = sum(len(v) for v in 未決.values())
        print(
            f"同じ原文の組 {len(重)} 件を {len(locs)} 言語で見ました"
            f"(まだ揃っていないと書いてあるのは {の数} 件)"
        )
    return bad


if __name__ == "__main__":
    raise SystemExit(main())
