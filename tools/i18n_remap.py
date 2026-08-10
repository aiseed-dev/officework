#!/usr/bin/env python3
"""訳の材料 `ui/i18n/<loc>.json` の**番号を振り直す**。

`gen_lang.py --todo` は `keys.json` を作り直すたびに番号を打ち直す。
各言語の材料は `{"i": 番号, "t": "訳"}` で番号を指しているので、
振り直さないと**全部の訳が1つずつずれた別の文に付く** — 気づきにくく、
気づいたときには13言語ぶん壊れている。

日本語の原文で突き合わせて移す(HIKITSUGI の「2ポインタ整列」)。
原文が変わった句の訳は**捨てる** — 文が変われば訳も当たらない。

    python3 tools/i18n_remap.py --old <古い keys.json>   # 振り直して残りを数える
    python3 tools/i18n_remap.py --todo <loc>             # まだ訳の無い句を出す
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
I18N = ROOT / "ui/i18n"
LOCALES = sorted(p.stem for p in I18N.glob("*.json") if p.stem != "keys")


def load(p):
    return json.loads(Path(p).read_text(encoding="utf-8"))


def save(p, obj):
    # **元の形のまま1行で書く。** 整形すると 12 個の材料が
    # 4,482 行に膨らんで、中身の差分が見えなくなる(2026-08-10 に一度やった)
    Path(p).write_text(json.dumps(obj, ensure_ascii=False), encoding="utf-8")


def remap(old_keys_path):
    old = {e["i"]: e["ja"] for e in load(old_keys_path)}
    new = {e["ja"]: e["i"] for e in load(I18N / "keys.json")}
    total = len(new)
    for loc in LOCALES:
        p = I18N / f"{loc}.json"
        got, lost, seen = [], 0, set()
        for e in load(p):
            ja = old.get(e["i"])
            if ja is None or ja not in new:
                lost += 1
                continue
            # **番号が重なることがある。** 別々だった2つの原文が同じ文に
            # 直されると、新しい番号は1つ。先に来たほうを採る(訳も同じ)
            if new[ja] in seen:
                continue
            seen.add(new[ja])
            got.append({"i": new[ja], "t": e["t"]})
        got.sort(key=lambda e: e["i"])
        save(p, got)
        print(f"{loc:6} {len(got):5} 訳 / 未訳 {total - len(got):4} / 原文が変わって捨てた {lost}")


def todo(loc):
    keys = load(I18N / "keys.json")
    have = {e["i"] for e in load(I18N / f"{loc}.json")}
    out = [e for e in keys if e["i"] not in have]
    print(json.dumps(out, ensure_ascii=False, indent=1))


def main():
    if "--old" in sys.argv:
        remap(sys.argv[sys.argv.index("--old") + 1])
        return 0
    if "--todo" in sys.argv:
        todo(sys.argv[sys.argv.index("--todo") + 1])
        return 0
    print(__doc__)
    return 1


if __name__ == "__main__":
    sys.exit(main())
