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
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
I18N = ROOT / "ui/i18n"
sys.path.insert(0, str(ROOT / "ui"))
import locales  # noqa: E402  綴りの正本
LOCALES = sorted(p.stem for p in I18N.glob("*.json") if p.stem != "keys")
for _l in LOCALES:
    if _l not in locales.TAGS:
        sys.exit(f"正本に無い材料があります: ui/i18n/{_l}.json"
                 f"(ui/locales.py の TAGS に足すか、名前を直す)")


def load(p):
    return json.loads(Path(p).read_text(encoding="utf-8"))


def save(p, obj):
    # **元の形のまま1行で書く。** 整形すると 12 個の材料が
    # 4,482 行に膨らんで、中身の差分が見えなくなる(2026-08-10 に一度やった)
    Path(p).write_text(json.dumps(obj, ensure_ascii=False), encoding="utf-8")


# 材料がどの keys.json に合わせて振ってあるかの印。**二度振りを止めるため**。
STAMP = I18N / ".remap-stamp"


def fingerprint(path) -> str:
    """keys.json の指紋。番号と原文の対だけを見る(訳や整形は関係ない)"""
    rows = [(e["i"], e["ja"]) for e in load(path)]
    return hashlib.sha256(repr(rows).encode("utf-8")).hexdigest()[:16]


def remap(old_keys_path):
    """材料の番号を、古い keys.json から**いまの** keys.json へ振り直す。

    **二度回してはいけない。** 一度目で新しい番号になった材料を、二度目が
    「古い番号」として読むと、番号は解決してしまうのに**別の文の訳が入る**。
    黙って12言語ぶんが広範にずれる — `要約` に "Ziel"、`縦書き` に "Ruby"
    のような壊れ方で、動かしても気づけない(2026-08-10 に実際に起きた)。

    だから**いまの keys.json の指紋を材料の隣に置き**、既にその指紋へ
    振ってあるなら断る。人が「もう一度念のため」と打っても壊れない。
    """
    now = fingerprint(I18N / "keys.json")
    if STAMP.exists() and STAMP.read_text(encoding="utf-8").strip() == now:
        sys.exit(
            "材料は既にいまの keys.json に合わせて振ってあります(指紋 "
            f"{now})。**二度振ると訳が別の文にずれます** — 何もしていません。\n"
            "本当にやり直すなら、材料を git で戻してから1度だけ回してください。"
        )
    if fingerprint(old_keys_path) == now:
        sys.exit(
            f"--old といまの keys.json が同じです(指紋 {now})。"
            "振り直すものがありません — 先に gen_lang.py --todo を回しましたか?"
        )
    old = {e["i"]: e["ja"] for e in load(old_keys_path)}
    # **同じ原文が2つの番号を持つことがある**(2026-08-10 時点で 12 句)。
    # 文言の表とリボンの語の両方に出る字で、材料では別の番号になる。
    # **これは事故ではなく、訳し分けるための自由度**  — 仏は 要約 を
    # ボタンで "Résumer"、文の中で "Résumé" と使い分けている。
    #
    # ここを `{ja: 番号}` の辞書にすると後の1つしか残らず、**文言側の訳が
    # リボン側の番号に書き込まれ、文言側は空になる**(訳が入れ替わる)。
    # 番号は「文言の塊 → リボンの塊」の順に振られるので、**同じ原文の中では
    # 順位で対応づける** — 1つ目は1つ目へ、2つ目は2つ目へ。
    old_ranks, new_ranks = {}, {}
    for i, ja in sorted(old.items()):
        old_ranks.setdefault(ja, []).append(i)
    for e in load(I18N / "keys.json"):
        new_ranks.setdefault(e["ja"], []).append(e["i"])
    for v in new_ranks.values():
        v.sort()
    total = sum(len(v) for v in new_ranks.values())
    moved = 0
    for loc in LOCALES:
        p = I18N / f"{loc}.json"
        got, lost, seen = [], 0, set()
        for e in load(p):
            ja = old.get(e["i"])
            if ja is None or ja not in new_ranks:
                lost += 1
                continue
            rank = old_ranks[ja].index(e["i"])
            dst = new_ranks[ja][min(rank, len(new_ranks[ja]) - 1)]
            if dst in seen:
                continue
            seen.add(dst)
            moved += dst != e["i"]
            got.append({"i": dst, "t": e["t"]})
        got.sort(key=lambda e: e["i"])
        save(p, got)
        print(f"{loc:6} {len(got):5} 訳 / 未訳 {total - len(got):4} / 原文が変わって捨てた {lost}")
    STAMP.write_text(now + "\n", encoding="utf-8")
    # **動いた番号の数を言う。** 0 なら振り直す必要が無かった(=間違えて
    # 回した)ということで、黙って通すと次の一手が危ない
    print(f"番号が動いた項目 {moved} 件。指紋 {now} を {STAMP.name} に控えました")
    if moved == 0:
        print("  ※ 1件も動いていません。--old の指定を確かめてください")


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
