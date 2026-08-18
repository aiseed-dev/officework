#!/usr/bin/env python3
"""手引きの「書き方の一覧」が、実物のボタンと揃っているかを見る。

手引きの表は「本文の書き方 / リボンのボタン / HTML」を1行に並べたもので、
**利用者はこの表を見てボタンを探します。** ボタンの札が変わったり段が
入れ替わったりすると、表は静かに嘘になります(押しても見つからない)。

見るのは3つです。

1. リボンの列に書いた「段 > ボタンの札」が実物にあるか
   (`face/src/ribbon.rs` の WRITER)
2. 本文の書き方の列に書いた印が、AsciiDoc の読み手にあるか
   (`engine/src/adoc.rs`。印そのものを探す — 表だけ増えるのを防ぐ)
3. 日本語版と英語版の行数が同じか(片方だけ増えると訳が抜ける)

    python3 tools/manual_table_check.py

**表が見つからなければ、この門番自身が落ちます。** 静かに0件と言うのは
いちばん悪い形です(2026-08-17 に writer_rows_check で踏みました)。
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
JA = ROOT / "docs/writer-manual.ja.adoc"
EN = ROOT / "docs/writer-manual.adoc"
RIBBON = ROOT / "face/src/ribbon.rs"
ADOC = ROOT / "engine/src/adoc.rs"

# 表の見出し行(この行の下から表が始まる)。**手引きは AsciiDoc です**
# (2026-08-18 に .md から移した)ので、表は `|===` で囲まれた形
HEAD_JA = "|したいこと |本文の書き方 |リボンのボタン |Web の形(HTML)"
HEAD_EN = "|What you want |Type this |Ribbon button |On the web (HTML)"


def rows(path, head):
    """表の行を(セルの並びで)返す。見つからなければ落ちる"""
    text = path.read_text(encoding="utf-8")
    if head not in text:
        sys.exit(f"{path} に「書き方の一覧」の表がありません(見出しの行が変わった?)")
    after = text.split(head, 1)[1].splitlines()
    out = []
    for line in after:
        if line.startswith("|==="):
            break  # 表の終わり
        if not line.startswith("|"):
            continue  # 表の中の空行
        # 表の中の `\|` は「棒そのもの」なので、そこでは割らない
        cells = [c.strip() for c in re.split(r"(?<!\\)\|", line.strip().strip("|"))]
        out.append(cells)
    if len(out) < 20:
        sys.exit(f"{path} の表が {len(out)} 行しかありません(表の取り方が壊れた?)")
    return out


def writer_buttons():
    """writer のリボン → {段の名前: [ボタンの札]}"""
    src = RIBBON.read_text(encoding="utf-8")
    seg = src[src.index("pub const WRITER"):src.index("pub const CALC")]
    tabs = {}
    now = None
    for line in seg.splitlines():
        m = re.search(r'Tab \{ name: "([^"]+)"', line)
        if m:
            now = m.group(1)
            tabs[now] = []
            continue
        m = re.search(r'c\("[^"]+",\s*"([^"]+)"', line)
        if m and now:
            tabs[now].append(m.group(1))
    if len(tabs) < 5:
        sys.exit("face/src/ribbon.rs から writer の段が読めません")
    return tabs


def main():
    ja = rows(JA, HEAD_JA)
    en = rows(EN, HEAD_EN)
    if len(ja) != len(en):
        sys.exit(f"日本語版 {len(ja)} 行、英語版 {len(en)} 行 — 行数が違います")

    tabs = writer_buttons()
    adoc_src = ADOC.read_text(encoding="utf-8")
    悪い = []

    for cells in ja:
        したいこと, 書き方, ボタン, _html = cells[0], cells[1], cells[2], cells[3]

        # ---- リボンの列 ----
        # 括弧で始まる注記(「(まだありません)」など)はボタンではない。
        # **ファイルの面と右パネルもここでは見ません** — どちらもリボンの表
        # (face/src/ribbon.rs)ではなく writer の画面の側(view.rs / panels.rs)に
        # 札があり、機械で突き合わせる相手がまだありません
        見る = (
            ">" in ボタン
            and not ボタン.startswith("(")
            and not ボタン.startswith("ファイル")
            and not ボタン.startswith("右パネル")
        )
        if 見る:
            段, *あと = [x.strip() for x in ボタン.split(">")]
            if 段 not in tabs:
                悪い.append(f"{したいこと}: 「{段}」という段がありません")
            elif あと and あと[0] not in tabs[段]:
                # 「段 > ボタン > 一覧の項目」の形なら、見るのは真ん中まで
                悪い.append(f"{したいこと}: 「{段}」に「{あと[0]}」のボタンがありません")

        # ---- 本文の書き方の列 ----
        # 印を1つ取って、読み手がその印を知っているか見る
        for 印 in ("footnote:", "ruby:", "field:", "stem:", "image::", "<<<"):
            if 印 in 書き方 and 印 not in adoc_src:
                悪い.append(f"{したいこと}: 手引きの「{印}」を adoc の読み手が知りません")

    if 悪い:
        print("手引きの「書き方の一覧」が実物と揃っていません:")
        for x in 悪い:
            print("  -", x)
        sys.exit(1)
    print(f"手引きの「書き方の一覧」は実物と揃っています({len(ja)} 行、日英で同数)")


if __name__ == "__main__":
    main()
