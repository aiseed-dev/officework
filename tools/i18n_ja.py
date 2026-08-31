#!/usr/bin/env python3
"""**日本語の文言を引く道**(2026-08-26 の移行のあと)。

鍵が英語になりました。日本語は `ui/i18n/ja.json` の訳です。

日本語のマニュアルと画面の字を突き合わせる道具
(`dialog_value_check.py`・`api_taiou.py`・`manual_table_check.py`・
`command_docs.py`)は、ソースから拾った鍵をそのまま比べられません。
**記号の鍵 → ja.json → 日本語の札**と一段引く必要があります。

引く所を1本にまとめておくのがこの綴りです。道具ごとに書くと、必ず
どれかが古びます。

    import i18n_ja
    i18n_ja.japanese(鍵)         # 鍵 → 日本語(無ければ鍵のまま)
    i18n_ja.english(鍵)          # 鍵 → 英語(無ければ鍵のまま)
    i18n_ja.screen_japanese()    # 画面に出る日本語の字を全部
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
I18N = ROOT / "ui/i18n"


def _table(lang: str = "ja"):
    """鍵 → その言語の文言。**一度読んで取っておきます。**"""
    if not hasattr(_table, "value"):
        _table.value = {}
    if lang not in _table.value:
        _table.value[lang] = json.loads(
            (I18N / f"{lang}.json").read_text(encoding="utf-8"))
    return _table.value[lang]


def japanese(keys: str) -> str:
    """鍵を日本語に。表に無ければ鍵をそのまま返します。"""
    return _table().get(keys, keys)


def english(keys: str) -> str:
    """鍵を英語に。表に無ければ鍵をそのまま返します。

    **対応表のボタンの列を英語と日本語の2つにするために足しました**
    (2026-08-30 発注者)。画面を英語で使っている人は日本語名で引けません。
    右の python-docx と openpyxl の列も英語なので、英語名が並ぶほうが
    突き合わせやすくもなります。
    """
    return _table("en").get(keys, keys)


def screen_japanese() -> set:
    """画面に出る日本語の字を全部。

    ja.json の訳(文言もリボンの語も入っています)に、リボンの札を足します。
    リボンの札は段2までコードの中に日本語で書いてあるので、そのまま拾えます。
    """
    out = set(_table().values())
    # **リボンの札は ribbon_ja.rs から。** 土台の ribbon.rs は英語です
    # (2026-08-26 の段2)
    ja = ROOT / "face/src/ribbon_ja.rs"
    if ja.exists():
        # 2026-08-31 の作り替え: ribbon_ja.rs は (英語の札, 語) の対だけ。
        # 語の側(タブ名も id 引きの例外も)を全部拾う
        import sys as _sys
        _sys.path.insert(0, str(ROOT / "tools"))
        import ribbon_parse
        w, by_id = ribbon_parse.words(ja)
        out |= set(w.values()) | set(by_id.values())
    return out
