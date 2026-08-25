#!/usr/bin/env python3
"""メニュー・API・python-docx・openpyxl の対応表を起こす。

発注者 2026-08-24「メニュー、API、python-docx、openpyxl の関係をまとめた
一覧表を作れ」。

1行が1つの操作です。画面のボタン、`officework` の呼び方、そして本家
(python-docx / openpyxl)の呼び方が横に並びます。

*引くための表なので、独立した1枚に置きます*(2026-08-24 発注者
「これが、インデックスの一つになるから、独立させないとダメでしょう」)。
手引きの中に埋めると、引きたい人が手引きを読む羽目になります。

    python3 tools/api_taiou.py           # 揃っているか見る(CI の検査)
    python3 tools/api_taiou.py --write   # 手引きの節を書き直す

対応は下の表が持ちます。**本家の側は実際に呼んで確かめた名前**です
(python-docx 1.2.0 / openpyxl 3.1.5)。無い所は空です。
"""
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).parent))
import ribbon_parse  # noqa: E402

# ボタンの id → (officework 文書, python-docx, officework 表, openpyxl)。
# **空文字はその道が無い。** id が両方の画面にあるときは1行にまとまります。
#
# *セルは文書の表にもあります*(2026-08-24 発注者)ので、
# 字・書式・結合・式はどちらの列も埋まります。
MICHI = {
    # ファイル
    "open": ("doc.Doc.open(径路)", "docx.Document(径路)", "sheet.Book.open(径路)", "load_workbook(径路)"),
    "save": ("d.save(径路)", "d.save(径路)", "b.save(径路)", "wb.save(径路)"),
    "pdf": ("", "", "", ""),
    # ホーム — 字と書式
    "copy": ("p.text = 値", "p.text = 値", "s['A1'] = 値", "ws['A1'] = 値"),
    "cut": ("p.text = 値", "p.text = 値", "s['A1'] = 値", "ws['A1'] = 値"),
    "paste": ("p.text = 値", "p.text = 値", "s['A1'] = 値", "ws['A1'] = 値"),
    "clear": ("r.clear()", "r.clear()", "s['A1'] = None", "ws['A1'] = None"),
    "bold": ("r.bold", "r.bold", "c.font", "c.font = Font(bold=True)"),
    "italic": ("r.italic", "r.italic", "c.font", "c.font = Font(italic=True)"),
    "underline": ("r.underline", "r.underline", "c.font", "c.font = Font(underline=…)"),
    "strikeout": ("r.strike", "r.font.strike", "", ""),
    "fontname": ("r.font", "r.font.name", "c.font", "c.font = Font(name=…)"),
    "fontsize": ("r.size_pt", "r.font.size", "c.font", "c.font = Font(size=…)"),
    "incfont": ("r.size_pt", "r.font.size", "c.font", "c.font = Font(size=…)"),
    "decfont": ("r.size_pt", "r.font.size", "c.font", "c.font = Font(size=…)"),
    "fontcolor": ("r.color", "r.font.color.rgb", "c.font", "c.font = Font(color=…)"),
    "superscript": ("", "r.font.superscript", "", ""),
    "subscript": ("", "r.font.subscript", "", "c.font = Font(vertAlign=…)"),
    "clearstyle": ("r.clear()", "r.clear()", "", ""),
    "ruby": ("", "", "", ""),
    "fillparag": ("", "", "c.fill", "c.fill = PatternFill(…)"),
    "borders": ("", "", "c.border", "c.border = Border(…)"),
    "align-left": ("p.align = 'left'", "p.alignment", "c.alignment", "c.alignment = Alignment(…)"),
    "align-center": ("p.align = 'center'", "p.alignment", "c.alignment", "c.alignment = Alignment(…)"),
    "align-right": ("p.align = 'right'", "p.alignment", "c.alignment", "c.alignment = Alignment(…)"),
    "align-just": ("p.align = 'justify'", "p.alignment", "", ""),
    "align-dist": ("p.align = 'distribute'", "", "", ""),
    "wrap": ("", "", "c.alignment", "c.alignment = Alignment(wrap_text=True)"),
    "merge": ("(表の col_span / v_merge)", "cell.merge(…)", "s.merge_cells('A1:B2')", "ws.merge_cells('A1:B2')"),
    "parastyle": ("p.style", "p.style", "", ""),
    "markers": ("p.style = '箇条書き'", "p.style = 'List Bullet'", "", ""),
    "numbering": ("p.style = '番号付き'", "p.style = 'List Number'", "", ""),
    "multilevels": ("", "", "", ""),
    "decoffset": ("p.paragraph_format", "p.paragraph_format.left_indent", "", ""),
    "incoffset": ("p.paragraph_format", "p.paragraph_format.left_indent", "", ""),
    "linespace": ("p.paragraph_format.line_spacing", "p.paragraph_format.line_spacing", "", ""),
    "replace": ("d.replace(前, 後)", "", "", ""),
    "format": ("", "", "c.number_format", "c.number_format"),
    "currency": ("", "", "c.number_format", "c.number_format"),
    "percents": ("", "", "c.number_format", "c.number_format"),
    "comma": ("", "", "c.number_format", "c.number_format"),
    "cell-ins": ("t.add_row()", "t.add_row()", "s.insert_rows(行)", "ws.insert_rows(行)"),
    "cell-del": ("", "", "s.delete_rows(行)", "ws.delete_rows(行)"),
    "condformat": ("", "", "", "ws.conditional_formatting.add(…)"),
    "sum": ("(表のセルに `=…`)", "", "s['A1'] = '=SUM(…)'", "ws['A1'] = '=SUM(…)'"),
    "defname": ("", "", "b.create_named_range(名前, …)", "wb.defined_names"),
    "sort-asc": ("", "", "", ""),
    "sort-desc": ("", "", "", ""),
    "setfilter": ("", "", "", "ws.auto_filter.ref"),
    "clear-filter": ("", "", "", "ws.auto_filter"),
    # 挿入
    "instable": ("d.add_table(行, 列)", "d.add_table(行, 列)", "s.add_table(…)", "ws.add_table(…)"),
    "insimage": ("d.add_picture(径路)", "d.add_picture(径路)", "", "ws.add_image(…)"),
    "inschart": ("", "", "", "ws.add_chart(…)"),
    "blankpage": ("d.add_page_break()", "d.add_page_break()", "", ""),
    "pagebreak": ("d.add_page_break()", "d.add_page_break()", "", "ws.row_breaks"),
    "edit-header": ("d.header / d.footer", "section.header", "s.oddHeader", "ws.oddHeader"),
    "edit-footer": ("d.footer", "section.footer", "", ""),
    "controls": ("d.fields()", "", "", ""),
    "insequation": ("", "", "", ""),
    "inshyperlink": ("", "", "c.hyperlink", "c.hyperlink"),
    "pivot-insert": ("", "", "", "ws.add_pivot(…)"),
    # レイアウト
    "pagemargins": ("d.sections[0]", "section.left_margin", "", "ws.page_margins"),
    "pageorient": ("d.sections[0]", "section.orientation", "", "ws.page_setup.orientation"),
    "pagesize": ("d.sections[0]", "section.page_width", "", "ws.page_setup.paperSize"),
    "printarea": ("", "", "s.print_area", "ws.print_area"),
    "printtitles": ("", "", "s.print_title_rows", "ws.print_title_rows"),
    "print-gridlines": ("", "", "s.print_gridlines", "ws.print_options.gridLines"),
    # 数式
    "insert-function": ("", "", "s['A1'] = '=…'", "ws['A1'] = '=…'"),
    "calc-mode": ("", "", "b.recalc()", "wb.calculation"),
    # データ
    "data-validation": ("", "", "s.add_data_validation(…)", "ws.add_data_validation(…)"),
    "group": ("", "", "s.row_groups", "ws.column_dimensions[…].outline_level"),
    "ungroup": ("", "", "s.row_groups", "ws.column_dimensions[…].outline_level"),
    # 参考資料
    "toc": ("", "", "", ""),
    "bookmarks": ("", "", "", ""),
    "crossref": ("", "", "", ""),
    "footnote": ("", "", "", ""),
    "caption": ("", "", "", ""),
    # フォーム
    "form-text": ("d.fill(名前, 値)", "", "", ""),
    "form-name": ("d.fields()", "", "", ""),
    # 共同編集
    "co-addcomment": ("p.add_comment(文, author=)", "p.add_comment(文)", "c.comment", "c.comment = Comment(…)"),
    "co-showcomment": ("d.comments", "d.comments", "c.comment", "c.comment"),
    # 保護
    "prot-doc": ("", "", "", "ws.protection"),
    "prot-encrypt": ("", "", "", "wb.security"),
    # 表示
    "freeze": ("", "", "s.freeze_panes", "ws.freeze_panes"),
    "show-gridlines": ("", "", "s.show_gridlines", "ws.sheet_view.showGridLines"),
}

MARK_S = "// api:taiou:start"
MARK_E = "// api:taiou:end"
SAKI = ROOT / "docs/api-taiou.ja.adoc"


def 段の並び(tabs):
    """**揃えた並び**(`face::tabs::merged` と同じ規則)。
    文章を軸にして、表だけの段をレイアウトの後ろへ入れます。"""
    w = [t.name for t in tabs["WRITER"]]
    c = [t.name for t in tabs["CALC"]]
    out = list(w)
    at = out.index("レイアウト") + 1 if "レイアウト" in out else len(out)
    for n in c:
        if n not in w:
            out.insert(at, n)
            at += 1
    return out


def rows():
    """(段, ボタン, 文書, python-docx, 表, openpyxl)。**メニューの並びのまま**"""
    tabs = ribbon_parse.tables_or_die()
    並び = 段の並び(tabs)
    w = {t.name: t for t in tabs["WRITER"]}
    c = {t.name: t for t in tabs["CALC"]}
    out = []
    for 段 in 並び:
        見た = set()
        for t in (w.get(段), c.get(段)):
            if t is None:
                continue
            for cmd in t.cmds:
                if not cmd.id or cmd.id in 見た or cmd.id not in MICHI:
                    continue
                見た.add(cmd.id)
                文, pd, 表c, op = MICHI[cmd.id]
                out.append((段, cmd.label, 文, pd, 表c, op))
    return out


def 表() -> str:
    r = rows()
    o = []
    o.append("1行が1つのボタンです。*並びは画面のメニューのまま*で、")
    o.append("`officework` の呼び方と、本家の呼び方が横に並びます。\n")
    o.append("*列は「うち・本家」の組を2つ*です。")
    o.append("左の2列が文書、右の2列が表。空いている所(—)は、その道がありません。\n")
    o.append("この節は `tools/api_taiou.py` が起こします。手で直さないでください。\n")
    いま = None
    for 段, ラベル, 文, pd, 表c, op in r:
        if 段 != いま:
            if いま is not None:
                o.append("|===\n")
            o.append(f"=== {段}")
            o.append("")
            o.append('[cols="2,3,3,3,3"]')
            o.append("|===")
            o.append("|ボタン |officework(文書) |python-docx |officework(表) |openpyxl\n")
            いま = 段
        f = lambda x: x if x else "—"
        o.append(f"|{ラベル} |{f(文)} |{f(pd)} |{f(表c)} |{f(op)}")
    if いま is not None:
        o.append("|===\n")
    return "\n".join(o)


def main() -> int:
    src = SAKI.read_text(encoding="utf-8")
    m = re.search(rf"({re.escape(MARK_S)}[^\n]*\n)(.*?)(\n?{re.escape(MARK_E)})", src, re.S)
    if not m:
        print(f"::error::{SAKI.name} に {MARK_S} の印がありません", file=sys.stderr)
        return 1
    beki = 表()
    if "--write" in sys.argv:
        SAKI.write_text(src[: m.start(2)] + beki + src[m.end(2):], encoding="utf-8")
        print(f"{SAKI.name} を書き直しました({len(rows())} 行)")
        return 0
    if m.group(2).strip() != beki.strip():
        # **手元では直します。落とすのは CI だけ**(2026-08-24 発注者
        # 「このような修正で検査が落ちないようにしろ」)。
        #
        # 生成物と道具がずれるのは、道具を直して `--write` を忘れたときです。
        # *それは機械が直せる*ので、手元では直して先へ進みます。
        # CI では直せません(直しても誰もコミットしない)ので、落として言います
        if os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS"):
            print(f"::error::{SAKI.name} の対応表が実物とずれています"
                  "(python3 tools/api_taiou.py --write で直してコミットしてください)",
                  file=sys.stderr)
            return 1
        SAKI.write_text(src[: m.start(2)] + beki + src[m.end(2):], encoding="utf-8")
        print(f"{SAKI.name} がずれていたので直しました({len(rows())} 行)。"
              "コミットに入れてください")
        return 0
    print(f"対応表は実物と揃っています({len(rows())} 行)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
