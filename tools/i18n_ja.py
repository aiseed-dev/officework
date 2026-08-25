#!/usr/bin/env python3
"""**日本語の文言を引く道**(2026-08-26 の移行のあと)。

鍵が英語になりました。日本語は `ui/i18n/ja.json` の訳です。

日本語のマニュアルと画面の字を突き合わせる道具
(`dialog_value_check.py`・`api_taiou.py`・`manual_table_check.py`・
`command_docs.py`)は、ソースから拾った鍵をそのまま比べられません。
**英語の鍵 → ja.json → 日本語の札**と一段引く必要があります。

引く所を1本にまとめておくのがこの綴りです。道具ごとに書くと、必ず
どれかが古びます。

    import i18n_ja
    i18n_ja.日本語(鍵)        # 英語の鍵 → 日本語(無ければ鍵のまま)
    i18n_ja.画面の日本語()     # 画面に出る日本語の字を全部
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
I18N = ROOT / "ui/i18n"


def _表():
    """英語の鍵 → 日本語。**一度読んで取っておきます。**"""
    if not hasattr(_表, "値"):
        鍵 = json.loads((I18N / "keys.json").read_text(encoding="utf-8"))
        訳 = {x["i"]: x["t"]
              for x in json.loads((I18N / "ja.json").read_text(encoding="utf-8"))}
        _表.値 = {k["key"]: 訳.get(k["i"], k["key"]) for k in 鍵}
    return _表.値


def 日本語(鍵: str) -> str:
    """英語の鍵を日本語に。表に無ければ鍵をそのまま返します。"""
    return _表().get(鍵, 鍵)


def 画面の日本語() -> set:
    """画面に出る日本語の字を全部。

    ja.json の訳(文言もリボンの語も入っています)に、リボンの札を足します。
    リボンの札は段2までコードの中に日本語で書いてあるので、そのまま拾えます。
    """
    出 = set(_表().values())
    ribbon = ROOT / "face/src/ribbon.rs"
    if ribbon.exists():
        import re
        出 |= set(re.findall(r'c\("[^"]*",\s*"([^"]+)"',
                             ribbon.read_text(encoding="utf-8")))
    return 出
