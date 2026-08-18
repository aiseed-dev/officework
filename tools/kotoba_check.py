#!/usr/bin/env python3
"""**古めかしい言い方**が文書に残っていないか調べます。

2026-08-08 から4回、発注者から同じ指摘を受けています。禁止する言葉の表を
作っても、書くたびに新しい比喩を思いついてしまうので、機械に見てもらいます。

    python3 tools/kotoba_check.py

見るのは**利用者が読む文書**です。設計の記録(docs/sekkei/)と機能の対応表は
経緯を残す場所なので見ません。過去に書いたものまで直すと、記録が読めなく
なるためです。

判定に迷う言葉(「目的」の「的」、「便利」の「便」など)は、間違って
拾わないように、はっきりした形だけを見ます。**黙って見逃すほうが、
関係ない所で騒ぐより害が少ない**からです。
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# 見る文書。利用者が読むものだけ
TARGETS = [
    "README.adoc", "README.ja.adoc", "CLAUDE.md",
    "docs/*manual*.adoc", "docs/from-excel*.adoc", "docs/engine*.adoc",
    "docs/mac-signing.ja.adoc", "packaging/README.ja.md", "pysheet/README.md",
    "sample/README.md",
]

# 見ないもの(経緯を残す場所)
SKIP = ("docs/sekkei/", "guide-tsukiawase", ".flatpak-builder/")

# はっきり言い換えられる言葉。正規表現 → 言い換え
WORDS = [
    (r"台本", "スクリプト"),
    (r"門番", "チェック、検査"),
    (r"物差し", "基準、確認方法"),
    (r"巻物", "連続表示"),
    (r"組み手|折り手", "レイアウト処理"),
    (r"名乗り", "宣言"),
    (r"束縛", "変数"),
    (r"語彙", "書き方"),
    (r"正本", "元、原本"),
    (r"素の", "普通の、元の"),
    (r"着せ(る|ない|た)", "使う、使わない"),
    (r"実測", "実際に動かして確かめた"),
    (r"釦", "ボタン"),
    (r"檻", "サンドボックス"),
    (r"錨", "アンカー"),
    (r"生成器", "生成スクリプト"),
    (r"家訓", "方針"),
    # **設定やキーボードの「キー」を「鍵」と書かない**(2026-08-18 発注者)。
    # 暗号の鍵(公開鍵・秘密鍵・鍵ファイル)と、錠前の意味の「鍵ではありません」は
    # 普通の日本語なので拾わない
    (r"(?<!公開)(?<!秘密)(?<!署名の)(?<!打)鍵(?!ファイル)(?!ではありません)(?!束)(?!盤)", "キー"),
    # 形がはっきりしているものだけ拾う
    (r"別便|この便|次の便|同じ便", "回、作業"),
    (r"[のる]口(?=[はをにがで、。\s])", "API、ソケット、入り口"),
    (r"の的(?=[はをにがで、。\s])", "対象、対象 OS"),
    (r"を畳(む|んだ|んで)", "閉じる、削除する"),
]

# この文書自体は、言い換えの表を載せているので見逃します
ALLOW = {"CLAUDE.md", "tools/kotoba_check.py"}

# 語ごとの見逃し。**その文書では正しい日本語**なので拾いません
# (mac-signing は最初から最後まで署名の鍵の話です)
ALLOW_WORD = {"鍵": {"docs/mac-signing.ja.adoc"}}


def files():
    seen = []
    for pat in TARGETS:
        for p in sorted(ROOT.glob(pat)):
            rel = p.relative_to(ROOT).as_posix()
            if p.is_file() and rel not in ALLOW and not any(s in rel for s in SKIP):
                seen.append((rel, p))
    return seen


def main():
    hits = []
    for rel, p in files():
        for n, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
            for pat, better in WORDS:
                m = re.search(pat, line)
                if m and rel in ALLOW_WORD.get(m.group(0), set()):
                    continue
                if m:
                    hits.append((rel, n, m.group(0), better, line.strip()[:60]))
    if hits:
        print(f"古めかしい言い方が {len(hits)} 箇所あります。")
        print("普通の言葉に直してください(CLAUDE.md の表)。\n")
        for rel, n, word, better, line in hits:
            print(f"  {rel}:{n}  「{word}」→ {better}")
            print(f"      {line}")
        return 1
    print(f"文書 {len(files())} 枚、古めかしい言い方はありません")
    return 0


if __name__ == "__main__":
    sys.exit(main())
