"""ロケールの綴りの正本 — **この1枚以外に綴りを書かない**(2026-08-13)。

それまで綴りが4種あった: 材料のファイル名 `pt-BR`、Rust の札 `pt-br`、
module 名 `pt_br`、本家のファイル `pt-pt`/`pt`。どれが正か決まっておらず、
足すたびに変換をその場で書いていた(mod_name の小文字落としは 2026-08-11 に
CI の警告で気づいて足した)。

決め:
- **札(tag)= 小文字・ハイフン**。Rust の LANGS・settings.toml・
  材料 `ui/i18n/<tag>.json` の名前、すべてこれ
- module 名は派生(`-` → `_`)。ここの `module()` だけが変換を知る
- 本家(ONLYOFFICE)の綴りが違うものは `VENDOR` に書く —
  ポルトガル語は札の意味が逆(向こうの `pt.json` はブラジル、
  `pt-pt.json` が欧州。2026-08-11 発注者確定: こちらは
  「分岐しているブラジルだけに札を付ける」ので pt=欧州 / pt-br=ブラジル)

ja は鍵そのものなので載らない。言語を足すときはここへ1行 —
gen_lang.py が材料の名前も module も登録簿もここから起こす。
"""

# 表の揃った言語。並びは Rust の LANGS と同じ辞書順。
# **ja も載ります**(2026-08-26)。鍵が英語になったので、日本語も
# 訳の1つです。鍵そのものの言語だった en は表を持ちません
TAGS = ["de", "en", "es", "fr", "id", "it", "ja", "ko",
        "pt", "pt-br", "ru", "tr", "vi", "zh", "zh-tw"]

# 本家のロケールのファイル名(こちらの札と違うものだけ)
VENDOR = {"pt": "pt-pt", "pt-br": "pt"}


def module(tag: str) -> str:
    """札 → Rust の module 名。`pt-br` → `pt_br`(小文字は札の側で保証)"""
    return tag.replace("-", "_")


def canonical(raw: str) -> str:
    """入力の揺れ(pt-BR・PT_BR など)を札に落とす。知らない札は止める —
    黙って受けると、綴り違いのファイルが静かにもう1組できる"""
    tag = raw.strip().lower().replace("_", "-")
    if tag not in TAGS:
        raise SystemExit(f"知らないロケール札です: {raw}(正本 ui/locales.py の TAGS: {TAGS})")
    return tag
