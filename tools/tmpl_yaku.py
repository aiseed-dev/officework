#!/usr/bin/env python3
"""テンプレートの言葉の訳を、本家と LibreOffice から引いて `ui/i18n` に入れる。

    python3 tools/tmpl_yaku.py --lo <LibreOffice の訳を取り出した場所>
    python3 tools/tmpl_yaku.py --lo /tmp/lo --write     # 実際に書き込む

## 出どころの決め

SEKKEI(2026-08-21)のとおり、まず本家(ONLYOFFICE、`vendor/web-apps`)を
引きます。本家に無い語だけ LibreOffice の公式訳から取ります。どちらにも
無い語は**英語のまま残します** — 訳を自分で作りません。

## 引く綴りを決めた手だて

うちの日本語(`ui/gen_tmpl_words.py` の WORDS)は表の見出し用に短くして
あるので、出どころの英語とそのままでは合いません。そこで**日本語で照合**
しました。出どころの日本語がうちの日本語と同じ意味なら、その英語の綴りで
13 言語を引いてよい、と決めています。決めた綴りが下の `HIKU` です。

## LibreOffice の訳の取り出し方

言語パックは apt から取れます(入れる必要はありません)。

    apt-get download libreoffice-l10n-de libreoffice-l10n-fr ...
    dpkg-deb -x libreoffice-l10n-de_*.deb /tmp/lo/de

`/tmp/lo/<言語>/usr/lib/libreoffice/program/resource/*/LC_MESSAGES/*.mo`
を読みます。
"""
import argparse
import gettext
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
I18N = ROOT / "ui/i18n"
HONKE = ROOT / "vendor/web-apps/apps"

LOCS = ["de", "es", "fr", "id", "it", "ko", "pt", "pt-br", "ru", "tr", "vi", "zh", "zh-tw"]
# 出どころごとの言語の綴り
HONKE_LOC = {"pt-br": "pt", "pt": "pt-pt"}
LO_LOC = {"zh": "zh-cn"}
APPS = ["spreadsheeteditor", "documenteditor", "presentationeditor", "pdfeditor"]

# **記号 → 出どころで引く英語の綴り。**
#
# `None` は「どちらにも無い」。英語のまま残します。
HIKU = {
    "col_width": "Column width",
    "row_height": "Row height",
    "page_break": "Page Break",
    "view": "View",
    "tmpl_group": "Group",
    "tmpl_protect": "Protection",
    "format": "Format",
    "format_applied": "Apply to",
    "workbook": "Workbook",
    "tmpl_column": "Column",
    "row": "Row",
    "size": "Size",
    "gridlines": "Gridlines",
    "tmpl_zoom": "Zoom",
    "scale": "Scale",
    "fit_to_width": "Fit to width",
    # 「縦に収める」— 本家も LibreOffice も1語では持っていません
    "fit_to_page": "Fit to page",
    "fit_to_height": None,
    # 画面の「表示」タブの見出しの札。行番号と列番号のことです
    "row_col_headings": "Headings",
    # LibreOffice の「繰り返す行 / 繰り返す列」。**対で取ります** —
    # 片方を `Title row`、片方を `Columns to Repeat` から取ると、
    # 並んだときに言い方が揃いません(2026-08-28)
    "title_rows": "Rows to Repeat",
    "title_cols": "Columns to Repeat",
    "tmpl_text": "Text",
    "freeze": "Freeze panes",
    "rtl": "Right to left",
    "tab_color": "Tab color",
    "default_2": "Default",
    "default_col_width": None,
    "default_row_height": None,
    "kind": "Type",
    "level": "Level",
    "tmpl_collapsed": "Collapse",
    "item": "Item",
    "range": "Range",
    "even_page": "Even page",
    "first_page": "First page",
    "all_pages": "All",
    "header_even": None,
    "footer_even": None,
    "header_first": None,
    "footer_first": None,
    "theme_colors": "Theme colors",
    "show_r1c1": "R1C1 reference style",
    "edit_objects": "Edit objects",
    "tmpl_borders": "Borders",
    "halign": "Horizontal Alignment",
    "valign": "Vertical alignment",
    # `Fill Background` は露・仏で英語が半分残ります。`Background` で引きます
    "fill_bg": "Background",
    "fill_pattern": "Pattern",
    # 「塗りのテーマ色」「文字のテーマ色」は、どちらも1語では
    # 「テーマの色」にしかならず、2つが同じ字になります。英語のまま
    "fill_theme": None,
    "color_theme": None,
    "font_color": "Font color",
    "tmpl_font": "Font",
    "shrink": "Shrink to fit",
    # 「ロック解除」は本家が Locked しか持たず、引くと意味が裏返ります。
    # LibreOffice の「保護されていない」が同じ意味です
    "unlocked": "Not protected",
    "hide_formula": "Hide formula",
    # 線種の「中」は、既に訳のある medium_* から共通の言葉を取ります。
    # 取れない言語(仏)は `Medium` を引きます
    "medium": ("Medium", ["medium_dashed", "medium_dash_dot", "medium_dash_dot_dot"]),
    "diagonal": "Diagonal",
    "selection": "Selection",
    "slant_dash_dot": None,
    "align_general": "General",
    "center": "Center",
    "center_across": None,
    "edge_top": "Top Border",
    "edge_bottom": "Bottom Border",
    "edge_left": "Left Border",
    "edge_right": "Right Border",
}


# **本家の訳が誤っている所。** その言語だけ LibreOffice から取ります。
#
# SEKKEI(2026-08-21)の「本家が誤訳のときは LibreOffice の公式訳」の実際です。
# 2026-08-27 に、本家と LibreOffice を全語・全言語で突き合わせて見つけました。
# 値は (どこが誤りか, 代わりに引く英語)。**代わりの綴りが `None` なら、
# 同じ綴りを LibreOffice から引きます。**
HONKE_WRONG = {
    # 「行の高さ」が「列の高さ」になっています(行と列が逆)
    ("row_height", "zh"): ("行と列が逆", None),
    # 「テーマ色」の3字目が抜けています(主題顏 → 佈景主題色彩)
    ("theme_colors", "zh-tw"): ("字が足りない", None),
    # 「倍率」が「大きさ」と同じ字になり、size と見分けが付きません
    ("scale", "ko"): ("size と同じ字になる", None),
    # 「倍率」が「寸法」の意味になっています
    ("scale", "zh"): ("大きさの意味になっている", None),
    # 見出しに使えない形(対格)です。Строку → Строка
    ("row", "ru"): ("格が違う", None),
    # 「ブック」が「本の中で」になっています。露語だけ `Book` で引きます
    # (独語の `Book` は「本」になるので、綴りの差し替えは露語だけ)
    ("workbook", "ru"): ("「本の中で」になっている", "Book"),
}


def honke(loc):
    """本家の (英語 → その言語) の表。同じ英語に複数あれば多い方を採る"""
    pairs = {}
    for app in APPS:
        for kind in ["main", "mobile", "embed"]:
            en_p = HONKE / app / kind / "locale/en.json"
            lo_p = HONKE / app / kind / f"locale/{loc}.json"
            if not en_p.exists() or not lo_p.exists():
                continue
            en = json.loads(en_p.read_text(encoding="utf-8"))
            lo = json.loads(lo_p.read_text(encoding="utf-8"))

            def walk(a, b):
                if isinstance(a, dict) and isinstance(b, dict):
                    for k in a:
                        if k in b:
                            walk(a[k], b[k])
                elif isinstance(a, str) and isinstance(b, str):
                    pairs.setdefault(a.strip(), {}).setdefault(b.strip(), 0)
                    pairs[a.strip()][b.strip()] += 1

            walk(en, lo)
    return {k: max(v, key=v.get) for k, v in pairs.items()}


def libre(base, loc):
    """LibreOffice の (英語 → その言語) の表"""
    dirs = list((base / loc).glob("usr/lib/libreoffice/program/resource/*/LC_MESSAGES"))
    if not dirs:
        return {}
    out = {}
    for f in sorted(dirs[0].glob("*.mo")):
        try:
            with open(f, "rb") as fh:
                cat = gettext.GNUTranslations(fh)._catalog
        except Exception:
            continue
        for k, v in cat.items():
            if isinstance(k, str) and k:
                key = k.split("\x04", 1)[1] if "\x04" in k else k
                out.setdefault(clean(key), clean(v))
    return out


# 訳さなくてよい語(記号や商標)。これが残っていても未訳とはしません
SONOMAMA = {"zoom", "text", "format", "item", "level", "type", "r1c1"}
# ラテン文字で書く言語。ここに他の文字体系が混ざっていたら字化けです
LATIN = {"de", "es", "fr", "id", "it", "pt", "pt-br", "tr", "vi"}


def moji_ga_majitteiru(v, loc):
    """別の文字体系が混ざっていないか。

    ポルトガル語の `Тexto` は、頭の T がキリル文字です(2026-08-27 に
    実物で見つけた本家の穴)。字が同じ形をしているので目では気づけません。
    """
    return loc in LATIN and any("\u0400" <= c <= "\u04ff" or "\u3000" <= c for c in v)


def eigo_no_mama(v, en):
    """英語のまま残っているか。

    **これだけでは落としません。** その言語にも同じ綴りの語があります
    (フランス語の Protection、スペイン語の General)。もう一方の出どころが
    違う訳を持っているときだけ、そちらへ乗り換えます。
    """
    return v.strip().lower() == en.strip().lower()


def clean(s):
    """下線や波線の押しキーの印を落とす。`保存(_S)` → `保存`"""
    s = re.sub(r"[_~]", "", s)
    s = re.sub(r"\s*\([A-Za-z]\)\s*$", "", s)
    return s.strip()


def common_of(words):
    """並びに共通の言葉を取り出す。

    修飾の付く場所は言語によって違います。ドイツ語は頭
    (`Mittel gestrichelt`)、スペイン語は尻(`Discontinua media`)、
    中国語は頭で切れ目が無い(`中粗虚线`)。頭と尻の両方を見ます。
    """
    if not words or any(not w for w in words):
        return None
    # 空白で分かれる言語は語ごと、分かれない言語(中国語など)は字ごと
    split = [w.split() for w in words]
    if all(len(x) == 1 for x in split):
        split = [list(w) for w in words]
        joint = ""
    else:
        joint = " "
    for take_head in (True, False):
        n = min(len(x) for x in split)
        out = []
        for i in range(n):
            j = i if take_head else -1 - i
            if len({x[j] for x in split}) == 1:
                out.append(split[0][j])
            else:
                break
        if out:
            if not take_head:
                out.reverse()
            got = joint.join(out).strip()
            # 1字だけの共通は当てになりません(「の」だけ、など)
            if len(got) >= 2:
                return got
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--lo", type=Path, required=True, help="LibreOffice の訳を取り出した場所")
    ap.add_argument("--write", action="store_true", help="ui/i18n に書き込む")
    args = ap.parse_args()

    ja = json.loads((I18N / "ja.json").read_text(encoding="utf-8"))
    tsuka = {}
    nokori = set()
    for loc in LOCS:
        cur = json.loads((I18N / f"{loc}.json").read_text(encoding="utf-8"))
        oo = honke(HONKE_LOC.get(loc, loc))
        lo = libre(args.lo, LO_LOC.get(loc, loc))
        tuita = 0
        for sym, how in HIKU.items():
            if sym in cur:
                continue
            if how is None:
                nokori.add(sym)
                continue
            if isinstance(how, tuple):
                v = common_of([cur.get(k, "") for k in how[1]])
                moto = "既にある訳から"
                if not v:
                    # 共通の言葉が取れない言語(修飾が語尾で形を変える)は
                    # 出どころから引きます
                    v = oo.get(how[0]) or lo.get(how[0])
                    moto = "本家 / LibreOffice"
            else:
                # **この語だけの差し替え。** 表の外へ持ち出さないよう、
                # 出どころは別の名前で受けます
                oo1 = oo
                if (sym, loc) in HONKE_WRONG:
                    _, kawari = HONKE_WRONG[(sym, loc)]
                    if kawari:
                        how = kawari      # 別の綴りで引き直します
                    else:
                        oo1 = {}          # 同じ綴りを LibreOffice からだけ
                v, moto = oo1.get(how), "本家"
                hoka = lo.get(how)
                if v and moji_ga_majitteiru(v, loc) and hoka:
                    v, moto = hoka, "LibreOffice(本家の字が混ざる)"
                elif v and eigo_no_mama(v, how) and hoka and not eigo_no_mama(hoka, how):
                    # 本家が英語のまま。LibreOffice が訳しているならそちら
                    v, moto = hoka, "LibreOffice(本家が未訳)"
                if not v:
                    v, moto = lo.get(how), "LibreOffice"
                if not v:
                    # 出どころによって頭が大文字だったりします。
                    # **同じ綴りの大小違いだけ**を受けます(別の語は見ません)
                    for cat, name in [(oo1, "本家"), (lo, "LibreOffice")]:
                        for k, x in cat.items():
                            if k.lower() == how.lower():
                                v, moto = x, name
                                break
                        if v:
                            break
            if v and isinstance(how, str) and moji_ga_majitteiru(v, loc):
                v = None  # 字が混ざっています。英語のまま残します
            if not v:
                nokori.add(sym)
                continue
            cur[sym] = v
            tsuka[moto] = tsuka.get(moto, 0) + 1
            tuita += 1
        print(f"{loc}: {tuita} 語を入れました")
        if args.write:
            # **並びも字下げも元のまま。** 鍵を並べ替えると、訳を1語
            # 足しただけで 2,000 行の差分になり、何を足したか読めません
            # (2026-08-27 に一度やってしまいました)
            (I18N / f"{loc}.json").write_text(
                json.dumps(cur, ensure_ascii=False, indent=1) + "\n",
                encoding="utf-8",
            )
    print("\n出どころ:", ", ".join(f"{k} {v}" for k, v in sorted(tsuka.items(), key=lambda kv: -kv[1])))
    if nokori:
        print(f"\n英語のまま残した語 {len(nokori)}(出どころに無い語です):")
        for s in sorted(nokori):
            print(f"  {s:22} {ja.get(s, '')}")
    if not args.write:
        print("\n(--write を付けると書き込みます)")


if __name__ == "__main__":
    main()
