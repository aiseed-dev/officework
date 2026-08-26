#!/usr/bin/env python3
"""**同じ日本語は1つの番号**(2026-08-22)。

officework の訳の材料は `ui/i18n/keys.json` で、番号がついています。
材料は2つの出どころから来ます。

* 文言 — `ui::t!` や `ui::item!` で使う語
* リボンの語 — `ui/gen_ribbon_locale.py` の `OVERRIDES`(本家に無いボタンの語)

同じ語が両方に載ることがあります。**番号を分けると訳も分かれます。**
2026-08-21 に「セルの書式設定」がリボンと保護の設定の一覧で別の語に
なっている言語が8つありました(スペイン語なら `Dar formato a celdas` と
`Aplicar formato a celdas`)。利用者から見れば同じ機能の名前なので、
揃っていないと探せません。

## 見張るものが変わりました(2026-08-22)

前はこの検査が「2つの番号の訳が揃っているか」を見て、人が揃えていました。
15 組ありました。いまは `ui/gen_lang.py` の `material()` が**同じ日本語を
1つの番号にまとめます**。揃えるより、分かれない方が確実です。

だからこの検査は「**重なりが1つも無いこと**」を見ます。重なりが出たら、
`material()` のまとめが効いていないということです。

英語が食い違うときはリボンの語を採ります — 利用者がボタンで読む名前なので、
案内の文もその名前で呼ぶのが筋です。まとめるのは `material()` の仕事で、
ここは結果を見るだけです。

## 使い方

    python3 tools/i18n_onaji_go_check.py
"""
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "ui"))
import locales  # noqa: E402


def main() -> int:
    keys = json.loads((ROOT / "ui/i18n/keys.json").read_text(encoding="utf-8"))
    if len(keys) < 500:
        # **読めなくなったら落ちる。** 静かに緑になるのが一番悪い
        print(f"::error::材料が {len(keys)} 句しかありません(keys.json の形が変わった?)")
        return 1

    # **鍵は英語です**(2026-08-26 の移行)。前は日本語で重なりを見て
    # いました。同じ鍵が2つの番号を持てば、訳も2つに分かれます
    group: dict[str, list[int]] = {}
    for i, k in enumerate(keys):
        group.setdefault(k["key"], []).append(i)
    重 = {w: v for w, v in group.items() if len(v) > 1}
    if 重:
        for w, idx in sorted(重.items()):
            print(
                f"::error::{w!r} が {len(idx)} つの番号を持っています(番号 {idx})。"
                "ui/gen_lang.py の material() が同じ日本語を1つにまとめるはずです"
            )
        return 1

    # 番号が1つでも、**訳が空のままでは意味がありません。**
    # 13 言語ぶん埋まっているかもここで見ます
    locs = [t for t in locales.TAGS if t != "en"]
    missing_ids: list[str] = []
    for loc in locs:
        p = ROOT / "ui/i18n" / f"{loc}.json"
        訳 = {k for k, v in json.loads(p.read_text(encoding="utf-8")).items() if v}
        欠け = len(keys) - len(訳 & {k["key"] for k in keys})
        if 欠け:
            missing_ids.append(f"{loc}: {欠け} 句")
    if missing_ids:
        print("::error::訳の空きがあります: " + " / ".join(missing_ids))
        print("  ui/gen_lang.py --todo で鍵を出し、ui/i18n/<言語>.json に書いてください")
        return 1

    print(f"材料 {len(keys)} 句に同じ日本語の重なりはありません({len(locs)} 言語とも訳は埋まっています)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
