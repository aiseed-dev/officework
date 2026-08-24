#!/usr/bin/env python3
"""原稿の中のシート(メモ用)に、どのボタンが要るかを表にする。

発注者 2026-08-24「calc のシートはシンプルにしたい。メモ用でいいのでは」。

*線を引くのは好みではなく、ファイルの形です。* `.sheet.adoc` が持てるのは
シート名・セルの値・式・結合だけで、書式・列幅・図形・ピボットは
`sheet::adoc` が落として「落とした」と言います(あちらの説明のとおり)。

だから**原稿のシートの画面には、`.sheet.adoc` が持てる物だけを出します**。
保存できない物のボタンを出すと、やった仕事が消えます。

    python3 tools/memo_sheet_table.py           # 一覧
    python3 tools/memo_sheet_table.py --adoc    # 設計に貼る形
    python3 tools/memo_sheet_table.py --check   # 元と食い違えば落ちる
"""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import ribbon_parse  # noqa: E402

# 原稿のシートに残すボタン。**`.sheet.adoc` が持てる物だけ**
NOKOSU = {
    # 値を入れる・動かす
    "copy": "値", "cut": "値", "paste": "値", "clear": "値",
    "fill-num": "値", "flash-fill": "値",
    "cell-ins": "値", "cell-del": "値",
    # 式
    "sum": "式", "insert-function": "式", "func-list": "式",
    "fn-recent": "式", "fn-financial": "式", "fn-logical": "式",
    "fn-text": "式", "fn-datetime": "式", "fn-lookup": "式",
    "fn-math": "式", "fn-more": "式",
    "defname": "式", "paste-name": "式", "show-formulas": "式",
    "calc-mode": "式", "trace-prec": "式", "trace-dep": "式",
    "remove-arrows": "式", "watch": "式",
    # 結合(adoc の表が持つ)
    "merge": "結合",
    # 並べ替え(値の並びが変わる = 保存される)
    "sort-asc": "値の並び", "sort-desc": "値の並び", "custom-sort": "値の並び",
    "rem-duplicates": "値の並び",
    # ファイル
    "open": "ファイル", "save": "ファイル", "pdf": "ファイル",
    # 探す
    "replace": "探す", "selectall": "探す",
    # Python(原稿の目的そのもの)
    "python": "Python", "py-new": "Python", "py-list": "Python",
    "py-folder": "Python", "rec-toggle": "Python", "ribbon-list": "Python",
    # データの入手(入手先の記録 = 原稿の目的)
    "data-from-text": "入手", "csv-kind": "入手",
    # コメント(やり取りの記録)
    "co-addcomment": "記録", "co-delcomment": "記録", "co-showcomment": "記録",
}

# 落とす理由。id ごとに書かず、まとめの札で持つ
RIYUU = {
    "見た目": "テンプレートの持ち場。`.sheet.adoc` は書式を持ちません",
    "画面": "文書は変わりません",
    "量の道具": "メモ用の範囲を超えます。大きい表は calc のブックで",
    "docx/xlsx だけ": "`.adoc` に居場所がありません",
}

OTOSU = {
    # 見た目
    **{k: "見た目" for k in [
        "copystyle", "fontname", "fontsize", "incfont", "decfont", "changecase",
        "bold", "italic", "underline", "strikeout", "subscript", "fontcolor",
        "fillparag", "borders", "top", "middle", "bottom", "wrap", "text-orient",
        "align-left", "align-center", "align-right", "align-just", "align-dist",
        "direction", "format", "currency", "percents", "comma", "digit-dec",
        "digit-inc", "cell-format", "condformat", "table-tpl", "cell-styles",
        "colorschemas", "pagemargins", "pageorient", "pagesize", "printarea",
        "pagebreak", "edit-header", "scale", "fit-pages", "printarea-add",
        "show-breaks", "printtitles", "rtl-sheet", "print-gridlines",
        "print-headings",
        "td-header", "td-total", "td-band-row", "td-first", "td-last",
        "td-band-col", "td-filter", "td-torange", "td-resize",
    ]},
    # 画面
    **{k: "画面" for k in [
        "sheet-view", "zoom-in", "zoom-out", "zoom100", "ui-bigger", "ui-smaller",
        "darkmode", "freeze", "split", "formula-bar", "show-gridlines",
        "show-headings", "show-zeros", "show-left", "show-right",
        "draw-select", "coauth-mode", "co-chat", "co-history",
    ]},
    # 量の道具
    **{k: "量の道具" for k in [
        "pivot-insert", "pivot-fields", "pivot-refresh", "pivot-refresh-all",
        "pivot-source", "pivot-chart", "pivot-select", "pivot-totals",
        "pivot-subtotals", "pivot-blank", "pivot-showas", "pivot-layout",
        "pivot-style", "insslicer", "insrecommend",
        "setfilter", "clear-filter", "group", "ungroup", "show-details",
        "hide-details", "subtotal", "datatable", "goal-seek", "scenario",
        "forecast", "solver", "text-column", "data-validation", "dv-mark",
        "data-external-links",
    ]},
    # docx/xlsx だけ
    **{k: "docx/xlsx だけ" for k in [
        "insimage", "insshape", "inssmartart", "inscheckbox", "inschart",
        "inssparkline", "inshyperlink", "instext", "instextart", "insequation",
        "inssymbol", "instable",
        "pen", "highlighter", "eraser",
        "prot-encrypt", "prot-doc", "prot-sign", "cell-lock", "prot-allow",
        "recover", "recover-every", "read-only-rec",
    ]},
}


def rows():
    tabs = ribbon_parse.tables_or_die()["CALC"]
    out = []
    for tab in tabs:
        for c in tab.cmds:
            if not c.id:
                continue
            if c.id in NOKOSU:
                out.append((tab.name, c.label, c.id, "残す", NOKOSU[c.id]))
            elif c.id in OTOSU:
                out.append((tab.name, c.label, c.id, "出さない", OTOSU[c.id]))
            else:
                out.append((tab.name, c.label, c.id, "*未分類*", ""))
    return out


def main() -> int:
    r = rows()
    未 = [x for x in r if x[3] == "*未分類*"]
    if 未:
        print("仕分けていないボタンがあります:", file=sys.stderr)
        for t, lb, i, _, _ in 未:
            print(f"  {t} {lb} ({i})", file=sys.stderr)
        return 1
    nokosu = [x for x in r if x[3] == "残す"]
    if "--check" in sys.argv:
        print(f"calc の {len(r)} ボタンは全部仕分けてあります(残す {len(nokosu)})")
        return 0

    if "--adoc" in sys.argv:
        print("*残すボタン*(`.sheet.adoc` が持てる物)。\n")
        print('[cols="1,2,1,1"]')
        print("|===")
        print("|タブ |ボタン |id |何のため\n")
        for t, lb, i, dou, w in nokosu:
            print(f"|{t} |{lb} |`{i}` |{w}")
        print("|===\n")
        print("*出さないボタン*(まとめ)。\n")
        print('[cols="1,1,3"]')
        print("|===")
        print("|理由 |数 |なぜ\n")
        for w, why in RIYUU.items():
            n = sum(1 for x in r if x[3] == "出さない" and x[4] == w)
            print(f"|{w} |{n} |{why}")
        print("|===")
    else:
        for t, lb, i, dou, w in r:
            print(f"{t:10} {lb:24} {i:18} {dou:6} {w}")
    print(f"\ncalc は {len(r)} ボタン。原稿のシートに残すのは {len(nokosu)} 個です。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
