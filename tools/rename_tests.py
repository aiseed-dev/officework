#!/usr/bin/env python3
"""試験の名前の変換表を作る(移行の段3の続き)。

    python3 tools/rename_tests.py            # 変換表を出す(書かない)
    python3 tools/rename_tests.py --write    # tools/rename_ids.json に足す
    python3 tools/rename_tests.py --files a  # そのファイルの分だけ

## 語の対応は、いま持っている対訳から起こします

手で語の表を書きません。材料は2つあります。

* `tools/rename_ids.json` — 段3で決めた識別子の対(874 組)。短い物は
  そのまま語の対応として使えます
* `ui/i18n/ja.json` ↔ `ui/i18n/en.json` — 画面の文言の対訳(2,177 組)。
  日本語の断片と英語の語の**共起**を測って対応を起こします

## 助詞と活用だけは規則で書きます

測ったところ、対訳だけでは 22% の字が切れませんでした。切れないのは
「は」「が」「を」「する」「した」といった**助詞と活用**です。画面の
文言は短い語が主で、文の対訳が少ないためです。ここは推定できないので、
下の表に書きます。**推定できる所と、書くしかない所を分けて持ちます。**

## 英語側は一意にします

同じ英語が2つの名前に付くと、同じ場所に2つの `fn` ができて通りません。
かぶったら後ろに数を足します。**一意でありさえすれば、プログラムとして
は壊れません** — 意味が合っているかは人が読んで確かめます。
"""
import collections
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import rename_ids as R  # noqa: E402

japanese = re.compile(r"[぀-ヿ一-鿿]+")
かな漢字 = re.compile(r"[぀-ヿ一-鿿]")

# **助詞と活用**。対訳からは起こせないので書きます。
# 値が空文字なら落とす、字があればその英語を当てる
助詞 = {
    "は": "", "が": "", "を": "", "に": "", "へ": "", "と": "", "の": "",
    "も": "", "で": "", "や": "", "から": "from", "まで": "to", "より": "than",
    "など": "etc", "だけ": "only", "ごと": "per", "ずつ": "each",
    "ても": "even_if", "でも": "even", "ば": "", "たら": "", "なら": "if",
    "ので": "so", "のに": "although", "けれど": "but", "が_": "",
}
活用 = {
    "します": "", "しました": "", "しない": "not", "しません": "not",
    "する": "", "した": "", "して": "", "せず": "not", "されない": "not",
    "される": "", "された": "", "できる": "can", "できない": "cannot",
    "ます": "", "ました": "", "ません": "not", "ない": "not", "なる": "becomes",
    "なった": "became", "ならない": "must_not", "ている": "", "ていない": "not",
    "ておく": "", "てある": "", "られる": "", "られない": "cannot",
    "たい": "", "よい": "ok", "べき": "should", "はず": "should",
    "です": "", "だ": "", "である": "", "でない": "not",
}

# 英語にしないでそのまま置く字(名前の中の英数字)
as_is = re.compile(r"[0-9A-Za-z_]+")

# 英語側で落とす語(名前が長くなりすぎるため)
薄い語 = {"the", "a", "an", "of", "to", "in", "on", "at", "for", "and",
          "is", "are", "be", "it", "its", "this", "that", "with", "from",
          "by", "as", "so", "you", "your", "we", "our", "they", "their"}


def 対訳():
    """(日本語, 英語) の組。画面の文言と、段3で決めた識別子の対。"""
    out = []
    ja = json.loads((ROOT / "ui/i18n/ja.json").read_text(encoding="utf-8"))
    en = json.loads((ROOT / "ui/i18n/en.json").read_text(encoding="utf-8"))
    for k, v in ja.items():
        if k in en:
            out.append((v, en[k]))
    ids = json.loads((ROOT / "tools/rename_ids.json").read_text(encoding="utf-8"))
    for a, b in ids.items():
        if b:
            out.append((a, b.replace("_", " ")))
    return out


def 英単語(s: str):
    return [w for w in re.split(r"[^0-9A-Za-z]+", s.lower())
            if w and w not in 薄い語]


def 語の表(group):
    """日本語の断片 → 英語の語。**共起の強さ**で選びます。

    Dice の係数(2×共起 /(片方ずつの数))で測り、いちばん強い英語を
    その断片の訳とします。1度しか一緒に出ない組は採りません — たまたま
    同じ文にあっただけの物を拾わないためです。
    """
    共起 = collections.Counter()
    ja数 = collections.Counter()
    en数 = collections.Counter()
    for sum, english in group:
        断片 = set()
        for m in japanese.finditer(sum):
            s = m.group(0)
            for n in range(1, 7):
                for i in range(len(s) - n + 1):
                    断片.add(s[i:i + n])
        word = set(英単語(english))
        for f in 断片:
            ja数[f] += 1
        for w in word:
            en数[w] += 1
        for f in 断片:
            for w in word:
                共起[(f, w)] += 1
    最良 = {}
    for (f, w), n in 共起.items():
        if n < 2:
            continue
        dice = 2 * n / (ja数[f] + en数[w])
        if dice < 0.25:
            continue
        if f not in 最良 or dice > 最良[f][1]:
            最良[f] = (w, dice)
    # 段3で決めた識別子の対は、そのまま語の対応として強く効かせます
    ids = json.loads((ROOT / "tools/rename_ids.json").read_text(encoding="utf-8"))
    for a, b in ids.items():
        if b and かな漢字.search(a) and len(a) <= 8:
            最良[a] = (b, 9.9)
    return {f: w for f, (w, _) in 最良.items()}


def cut(name: str, table):
    """名前を語に切って、英語の並びにする。切れない字は印を付けて返す。"""
    out, 不明 = [], []
    i = 0
    while i < len(name):
        # 英数字はそのまま
        m = as_is.match(name, i)
        if m:
            out.append(m.group(0).lower())
            i = m.end()
            continue
        hit = None
        for n in range(8, 0, -1):
            s = name[i:i + n]
            if len(s) < n:
                continue
            if s in 活用:
                hit = (s, 活用[s]); break
            if s in 助詞:
                hit = (s, 助詞[s]); break
            if s in table:
                hit = (s, table[s]); break
        if hit:
            if hit[1]:
                out.append(hit[1])
            i += len(hit[0])
        else:
            不明.append(name[i])
            i += 1
    return out, 不明


def 一意にする(cands: dict, 使用済み: set):
    """英語がかぶったら後ろに数を足す。**一意ならプログラムは壊れません。**"""
    out = {}
    for sum, english in cands.items():
        base = english or "test"
        name = base
        n = 2
        while name in 使用済み:
            name = f"{base}_{n}"
            n += 1
        使用済み.add(name)
        out[sum] = name
    return out


def main():
    table = 語の表(対訳())
    済み = {k: v for k, v in json.loads(
        (ROOT / "tools/rename_ids.json").read_text(encoding="utf-8")).items() if v}
    item = R.collect_into()
    rest = [k for k in item if k not in 済み]
    絞り = None
    if "--files" in sys.argv:
        絞り = sys.argv[sys.argv.index("--files") + 1]
        rest = [k for k in rest if any(絞り in f for f in item[k])]

    cands, 不明の字 = {}, collections.Counter()
    for name in sorted(rest):
        word, 不明 = cut(name, table)
        for c in 不明:
            不明の字[c] += 1
        # 長すぎる名前は 8 語で切ります(読める長さに収める)
        cands[name] = "_".join(word[:8])
    out = 一意にする(cands, set(済み.values()))

    print(f"語の表 {len(table)} 語 / 変換する名前 {len(out)} 本")
    if 不明の字:
        print(f"切れなかった字 {sum(不明の字.values())}"
              f"(種類 {len(不明の字)}): "
              + "".join(c for c, _ in 不明の字.most_common(30)))
    from_len = sum(1 for v in out.values() if not v.strip("_"))
    if from_len:
        print(f"!! 英語が空になった名前 {from_len} 本")
    if "--write" in sys.argv:
        p = ROOT / "tools/rename_ids.json"
        d = json.loads(p.read_text(encoding="utf-8"))
        d.update(out)
        p.write_text(json.dumps(d, ensure_ascii=False, indent=1) + "\n",
                     encoding="utf-8")
        print(f"tools/rename_ids.json に {len(out)} 本を足しました")
    else:
        for sum, english in list(out.items())[:30]:
            print(f"  {sum:32} {english}")
        print("(--write で書き込む)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
