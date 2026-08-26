#!/usr/bin/env python3
"""手引きの「ダイアログ」に書いた選べる値が、画面の文言と合っているか見る。

    python3 tools/dialog_value_check.py

発注者「ダイアログが出る場合は、パラメータがありますね。それを調べて
ください」(2026-08-25)。調べた結果は
`docs/sekkei/dialog-parameters.ja.adoc` の台帳にあり、そこから
`docs/ja/commands` の各頁へ流し込みました。

**台帳はコードから写した物なので、写し間違いが起きます。**
実際、流し込んだ回に3か所ずれていました。

* スパークラインの「縦棒」は、画面では「縦棒(カラム)」
* 紙に収めるの「すべての列を1ページ」は、画面では「…1ページに」
* 保護の許す操作は8個と書いていたが、画面は14個

利用者は画面に出ている字で探すので、頁と画面がずれていると引けません。
この道具は、頁の箇条書き(`* 値`)を画面の文言の一覧と突き合わせます。

見るのは*画面に出る字*の一覧です。

* `ui/i18n/ja.json` の訳(鍵は英語なので、一段引きます)
* リボンの札(`face/src/ribbon.rs`)

**説明の文は見ません。** 箇条書きのうち、句点で終わる物や長い物は
選べる値ではなく説明なので外します。
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import i18n_ja  # noqa: E402  英語の鍵 → 日本語の札

ROOT = Path(__file__).resolve().parent.parent
CMDS = ROOT / "docs/ja/commands"
除く = ("target", "vendor", ".flatpak-builder")

# 選べる値ではないもの。
# * 句点で終わる、記号で始まる → 説明の文
# * `欄の名前 — 説明` の形 → 欄の一覧であって、選べる値ではありません
_説明 = re.compile(r"[。、]$|^[|=*`]|—")


def 画面の文言() -> set:
    """**日本語のマニュアルと比べるので、日本語の側を集めます。**

    鍵が英語になったので(2026-08-26)、ソースの字をそのまま拾っても
    日本語の頁とは比べられません。`tools/i18n_ja.py` が一段引きます。
    """
    return i18n_ja.画面の日本語()


def 頁の値():
    """(頁, 値) を返す。ダイアログの節の箇条書きだけ見ます"""
    for p in sorted(CMDS.rglob("*.adoc")):
        if p.name == "README.ja.adoc":
            continue
        m = re.search(r"== ダイアログ\n\n(.*?)\n\n== ", p.read_text(encoding="utf-8"), re.S)
        if not m:
            continue
        for line in re.findall(r"^\* (.+)$", m.group(1), re.M):
            # 強調と註記を外す(`* 折れ線(*既定*)` → `折れ線`)
            v = re.sub(r"\(\*[^)]*\*\)|\*", "", line).strip()
            if len(v) < 3 or _説明.search(v) or len(v) > 30:
                continue
            yield str(p.relative_to(CMDS))[:-5], v


def main() -> int:
    文言 = 画面の文言()
    bad = [(f, v) for f, v in 頁の値() if v not in 文言]
    seen = sum(1 for _ in 頁の値())
    if not bad:
        print(f"手引きに書いた選べる値 {seen} 件、画面の文言と揃っています")
        return 0
    print(f"**画面に無い値が {len(bad)} 件あります。**", file=sys.stderr)
    print("利用者は画面の字で探します。頁を画面に合わせてください"
          "(画面のほうが間違っているなら、そちらを直します)。\n", file=sys.stderr)
    for f, v in bad:
        print(f"  {f}: 「{v}」", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
