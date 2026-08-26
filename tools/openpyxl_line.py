#!/usr/bin/env python3
"""calc のボタンを「openpyxl に道があるか」で仕分ける。

発注者 2026-08-24「calc に残すのは、openpyxl のコマンドがあるものだけに
したらどうですか。openpyxl との互換性がよくなるし」。

*測ったら、これは「ファイルに書ける物だけ残す」と同じ線でした。*
openpyxl は xlsx を読み書きするライブラリなので、持っている API は
xlsx に書ける物と一致します。ソルバーやゴールシークが無いのは、
機能が弱いからではなく**ファイルに書く物ではない**からです。

`.sheet.adoc` で引いた線(SEKKEI「原稿の中のシートは、メモ用にする」)と
同じ考えを、xlsx に当てたことになります。

    python3 tools/openpyxl_line.py           # 一覧
    python3 tools/openpyxl_line.py --adoc    # 設計に貼る形
    python3 tools/openpyxl_line.py --check   # 仕分け漏れがあれば落ちる

対応は下の表が持ちます。**実際に openpyxl 3.1.5 を呼んで確かめた**
機能に基づきます(書式・列幅・条件付き書式・データ検証・オートフィルタ・
アウトライン・枠の固定・印刷設定・ヘッダー/フッター・保護・表示の設定・
計算方法・名前・コメント・リンク・グラフ・画像・テーブル・ピボットは在り)。
"""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import ribbon_parse  # noqa: E402

# id → openpyxl の道
ARU = {
    "open": "load_workbook", "save": "Workbook.save",
    "copy": "cell.value", "cut": "cell.value", "paste": "cell.value",
    "clear": "cell.value = None", "fill-num": "cell.value",
    "cell-ins": "insert_rows / insert_cols", "cell-del": "delete_rows / delete_cols",
    "copystyle": "cell._style", "cell-styles": "NamedStyle",
    "fontname": "styles.Font", "fontsize": "styles.Font",
    "incfont": "styles.Font", "decfont": "styles.Font",
    "bold": "styles.Font", "italic": "styles.Font", "underline": "styles.Font",
    "strikeout": "styles.Font", "subscript": "styles.Font", "fontcolor": "styles.Font",
    "fillparag": "styles.PatternFill", "borders": "styles.Border",
    "top": "styles.Alignment", "middle": "styles.Alignment", "bottom": "styles.Alignment",
    "wrap": "styles.Alignment", "text-orient": "styles.Alignment",
    "align-left": "styles.Alignment", "align-center": "styles.Alignment",
    "align-right": "styles.Alignment", "align-just": "styles.Alignment",
    "align-dist": "styles.Alignment", "direction": "sheet_view.rightToLeft",
    "format": "number_format", "currency": "number_format",
    "percents": "number_format", "comma": "number_format",
    "digit-dec": "number_format", "digit-inc": "number_format",
    "cell-format": "styles(まとめて)",
    "condformat": "conditional_formatting",
    "table-tpl": "worksheet.tables", "instable": "add_table",
    "merge": "merge_cells", "changecase": "cell.value(字を直す)",
    "sum": "cell.value = '=SUM(…)'",
    "insert-function": "cell.value = '=…'", "paste-name": "defined_names",
    "fn-recent": "cell.value = '=…'", "fn-financial": "cell.value = '=…'",
    "fn-logical": "cell.value = '=…'", "fn-text": "cell.value = '=…'",
    "fn-datetime": "cell.value = '=…'", "fn-lookup": "cell.value = '=…'",
    "fn-math": "cell.value = '=…'", "fn-more": "cell.value = '=…'",
    "defname": "defined_names", "calc-mode": "workbook.calculation",
    "show-formulas": "sheet_view.showFormulas",
    "sort-asc": "値の並び", "sort-desc": "値の並び", "custom-sort": "値の並び",
    "setfilter": "auto_filter", "clear-filter": "auto_filter",
    "data-validation": "DataValidation", "dv-mark": "DataValidation",
    "group": "outline_level", "ungroup": "outline_level",
    "show-details": "outline_level", "hide-details": "outline_level",
    "insimage": "add_image", "inschart": "add_chart",
    "pivot-insert": "add_pivot", "pivot-fields": "pivot", "pivot-refresh": "pivot",
    "pivot-refresh-all": "pivot", "pivot-source": "pivot", "pivot-select": "pivot",
    "pivot-totals": "pivot", "pivot-subtotals": "pivot", "pivot-blank": "pivot",
    "pivot-showas": "pivot", "pivot-layout": "pivot", "pivot-style": "pivot",
    "pivot-chart": "pivot + add_chart",
    "td-header": "worksheet.tables", "td-total": "worksheet.tables",
    "td-band-row": "worksheet.tables", "td-band-col": "worksheet.tables",
    "td-first": "worksheet.tables", "td-last": "worksheet.tables",
    "td-filter": "worksheet.tables", "td-torange": "worksheet.tables",
    "td-resize": "worksheet.tables",
    "co-addcomment": "cell.comment", "co-delcomment": "cell.comment",
    "co-showcomment": "cell.comment", "inshyperlink": "cell.hyperlink",
    "pagemargins": "page_margins", "pageorient": "page_setup",
    "pagesize": "page_setup", "printarea": "print_area",
    "printarea-add": "print_area", "printtitles": "print_titles",
    "print-gridlines": "print_options", "print-headings": "print_options",
    "scale": "page_setup.scale", "fit-pages": "page_setup.fitToPage",
    "pagebreak": "row_breaks / col_breaks", "edit-header": "oddHeader / oddFooter",
    "rtl-sheet": "sheet_view.rightToLeft",
    "prot-doc": "worksheet.protection", "cell-lock": "styles.Protection",
    "prot-allow": "worksheet.protection", "prot-encrypt": "workbook.security",
    "freeze": "freeze_panes", "split": "sheet_view.pane",
    "show-gridlines": "sheet_view.showGridLines",
    "show-headings": "sheet_view.showRowColHeaders",
    "show-zeros": "sheet_view.showZeros",
    "zoom-in": "sheet_view.zoomScale", "zoom-out": "sheet_view.zoomScale",
    "zoom100": "sheet_view.zoomScale", "sheet-view": "sheet_view",
    "data-from-text": "値を入れる(読むのは Python)",
    "csv-kind": "値を入れる(読むのは Python)",
}

# id → 落とす理由
NAI = {
    # 計算の補助。**答えを出す道具で、ファイルには残りません**
    **{k: "計算の補助" for k in [
        "goal-seek", "scenario", "forecast", "solver", "datatable", "subtotal",
        "trace-prec", "trace-dep", "remove-arrows", "watch",
    ]},
    # 入力の補助。やった後に残るのは値だけ
    **{k: "入力の補助" for k in [
        "flash-fill", "text-column", "rem-duplicates", "replace", "selectall",
        "inssymbol",
    ]},
    # xlsx の飾り。openpyxl が持たない部品
    **{k: "openpyxl が持たない部品" for k in [
        "insshape", "inssmartart", "instext", "instextart", "inssparkline",
        "inscheckbox", "insslicer", "insrecommend", "insequation",
        "draw-select", "pen", "highlighter", "eraser",
        "prot-sign", "read-only-rec",
    ]},
    # 画面。ファイルに書かれません
    **{k: "画面" for k in [
        "ui-bigger", "ui-smaller", "darkmode", "formula-bar",
        "show-left", "show-right", "colorschemas", "show-breaks",
        "pdf", "recover", "recover-every",
    ]},
    # このアプリの物。openpyxl の外
    **{k: "このアプリの物" for k in [
        "python", "py-new", "py-list", "py-folder", "ribbon-list", "rec-toggle",
        "func-list", "coauth-mode", "co-chat", "co-history", "track-changes",
        "data-external-links",
    ]},
}


def rows():
    out = []
    for tab in ribbon_parse.tables_or_die()["CALC"]:
        for c in tab.cmds:
            if not c.id:
                out.append((tab.name, c.label, "(灰色)", "無い", "まだ作っていない"))
            elif c.id in ARU:
                out.append((tab.name, c.label, c.id, "ある", ARU[c.id]))
            elif c.id in NAI:
                out.append((tab.name, c.label, c.id, "無い", NAI[c.id]))
            else:
                out.append((tab.name, c.label, c.id, "*未分類*", ""))
    return out


def main() -> int:
    r = rows()
    未 = [x for x in r if x[3] == "*未分類*"]
    if 未:
        print("仕分けていないボタン:", file=sys.stderr)
        for t, lb, i, _, _ in 未:
            print(f"  {t} {lb} ({i})", file=sys.stderr)
        return 1
    ids = {x[2] for x in r if x[2] != "(灰色)"}
    aru = {x[2] for x in r if x[3] == "ある"}
    if "--check" in sys.argv:
        print(f"calc の {len(ids)} 種のボタンは全部仕分けてあります(openpyxl に道があるのは {len(aru)})")
        return 0

    if "--adoc" in sys.argv:
        print('[cols="1,1,3"]')
        print("|===")
        print("|落とす理由 |数 |中身\n")
        理由 = {}
        for t, lb, i, dou, w in r:
            if dou == "無い":
                理由.setdefault(w, set()).add(lb)
        for w, s in sorted(理由.items(), key=lambda kv: -len(kv[1])):
            inner = "・".join(sorted(s)[:8]) + ("ほか" if len(s) > 8 else "")
            print(f"|{w} |{len(s)} |{inner}")
        print("|===")
    else:
        for t, lb, i, dou, w in r:
            print(f"{t:10} {lb:26} {i:18} {dou:4} {w}")
    print(f"\ncalc は {len(ids)} 種。openpyxl に道があるのは {len(aru)}、無いのは {len(ids) - len(aru)} です。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
