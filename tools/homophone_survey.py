#!/usr/bin/env python3
"""実物の文書に**どの同音異義語が出ているか**を数え、濾過器の表の穴を出す。

**実装から入らないための道具**(SEKKEI docs/sekkei/nihongo.ja.md の 4)。

4(同音異義語の濾過器)の効きは、表がどれだけ実務の語を覆っているかで決まる。
だが「誤変換の入った文書」は集められない — 誤変換は publish される前に直るので、
青空文庫にも手引きにも出てこない。**個人情報と免許の問題もあり、repo にも置けない**
(docs/corpus.ja.md と同じ理由)。

**だから誤変換の文書は要らない。要るのは「読み」。**
文書に出てくる2字漢語を読みで束ね、**同じ読みの語が2つ以上そろって出ている**組を
数えれば、その書き手にとって紛らわしい語が出る。それが表に入るべき語。

読みは mecab-ipadic の生の CSV から取る。**これは測る道具の依存であって、
`lang` は何も持たない**(決めごと2「外部の辞書ファイルに依らない」は出荷する側の話。
青空文庫のコーパスと同じ立場)。

    sudo apt install mecab-ipadic-utf8     # 入っていなければ

使い方:

    python3 tools/homophone_survey.py docs/*.ja.md          # 文書を指定
    python3 tools/homophone_survey.py --all ~/文書/*.txt    # 表にある物も出す
    python3 tools/homophone_survey.py --docx ~/文書/*.docx  # docx から本文を抜く

出るのは「表に無いのに実際に出ている組」を多い順に並べた一覧。
**そのまま lang/src/ja/homophone.rs の KANGO へ足せる形**で出す。
"""

import csv
import glob
import os
import re
import sys
import zipfile
from collections import defaultdict

IPADIC = "/usr/share/mecab/dic/ipadic"
HOMOPHONE_RS = "lang/src/ja/homophone.rs"


def is_kanji(s):
    return s != "" and all(0x4E00 <= ord(c) <= 0x9FFF for c in s)


def readings():
    """ipadic から2字漢語の名詞の 表層形→読み を集め、読みで束ねる。

    固有名詞(人名・地名・組織名)は外す — 同じ読みの姓は無数にあり、
    紛らわしさの話ではない。
    """
    files = [
        p
        for p in glob.glob(os.path.join(IPADIC, "Noun*.csv"))
        if not any(k in p for k in ("name", "place", "org"))
    ]
    if not files:
        sys.exit(f"{IPADIC} に辞書がありません。apt install mecab-ipadic-utf8")
    by = defaultdict(set)
    for p in files:
        with open(p, encoding="euc-jp", errors="replace") as f:
            for row in csv.reader(f):
                # 表層形,左,右,コスト,品詞,細分類1-3,活用型,活用形,原形,読み,発音
                if len(row) < 12:
                    continue
                surface, yomi = row[0], row[11]
                if len(surface) == 2 and is_kanji(surface) and re.fullmatch(r"[゠-ヿ]+", yomi):
                    by[yomi].add(surface)
    return {y: sorted(v) for y, v in by.items() if len(v) >= 2}


def shipped():
    """いま出荷している表に載っている語。"""
    try:
        src = open(HOMOPHONE_RS, encoding="utf-8").read()
    except OSError:
        sys.exit(f"{HOMOPHONE_RS} が読めません。officework の根で動かしてください")
    body = src.split("const KANGO")[1].split("];")[0]
    return set(re.findall(r'"([^"]+)"', body))


def text_of(path, want_docx):
    """本文を取り出す。docx は w:t を繋ぐだけの粗い抜き方でよい(数えるだけ)。"""
    if want_docx and path.endswith(".docx"):
        with zipfile.ZipFile(path) as z:
            xml = z.read("word/document.xml").decode("utf-8", "replace")
        return " ".join(re.findall(r"<w:t[^>]*>([^<]*)</w:t>", xml))
    for enc in ("utf-8", "cp932"):
        try:
            return open(path, encoding=enc).read()
        except (UnicodeDecodeError, UnicodeError):
            continue
    return ""


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    show_all = "--all" in sys.argv
    want_docx = "--docx" in sys.argv
    if not args:
        sys.exit(__doc__)

    text = "".join(text_of(p, want_docx) for p in args)
    if not text:
        sys.exit("本文が取れませんでした")

    groups = readings()
    mine = shipped()

    # **2語以上そろって出ている**組だけが、その書き手にとって紛らわしい
    live = {}
    for yomi, words in groups.items():
        here = [w for w in words if w in text]
        if len(here) >= 2:
            live[yomi] = here

    covered = {y: w for y, w in live.items() if any(x in mine for x in w)}
    missing = {y: w for y, w in live.items() if y not in covered}

    print(f"本文 {len(text):,}字 / ipadic の2字漢語の同音異義語 {len(groups):,}組")
    print(f"この文書に2語以上そろって出る組: {len(live)}組")
    pct = len(covered) * 100 // max(len(live), 1)
    print(f"出荷中の表({len(mine)}語)が触れている組: {len(covered)}/{len(live)} = {pct}%")

    def count(words):
        return sum(text.count(w) for w in words)

    show = live if show_all else missing
    label = "実際に出ている組" if show_all else "表に無いのに実際に出ている組"
    print(f"\n--- {label}(多い順) ---")
    for yomi, words in sorted(show.items(), key=lambda kv: -count(kv[1])):
        mark = " " if yomi in covered else "+"
        print(f'{mark}   &[{", ".join(chr(34) + w + chr(34) for w in words)}],'
              f'   // {yomi} 計{count(words)}回')
    if not show:
        print("(無し)")
    print("\n+ の行は lang/src/ja/homophone.rs の KANGO にそのまま貼れる。")
    print("**貼ったら効きを測り直すこと** — 送らずに済む割合は必ず下がる(それが正常)。")


if __name__ == "__main__":
    main()
