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

# ボタンの id → (オブジェクト, officework, python-docx, openpyxl)。
# **officework は `.adoc` を触る1つの模型なので、文書と表で列を割りません**
# (2026-08-24 発注者)。どのオブジェクトの物かを示します。
# `A / B` は、いま2つの呼び方がある物です(寄せる仕事が残っています)。
MICHI = {
    "changecase": ("Run", "", "", ""),
    "inssymbol": ("Run", "", "", ""),
    "datetime": ("Paragraph", "", "", ""),
    "selectall": ("Doc", "", "", ""),
    "text-from-file": ("Doc", "", "", ""),
    "rem-duplicates": ("Sheet", "", "", ""),
    "flash-fill": ("Cell", "", "", ""),
    "text-column": ("Cell", "", "", ""),
    "subtotal": ("Cell", "", "", ""),
    "trace-prec": ("Cell", "", "", ""),
    "show-formulas": ("Cell", "", "", ""),
    "fill-num": ("Cell", "", "", ""),
    "numpages": ("Doc", "", "", ""),
    "pagenum": ("Doc", "", "", ""),
    "insrecommend": ("Sheet", "", "", ""),
    "func-list": ("Book", "", "", ""),
    "csv-kind": ("Sheet", "", "", ""),
    "data-from-text": ("Sheet", "", "", ""),
    "open": ("Doc / Book", "Doc.open(径路) / Book.open(径路)", "docx.Document(径路)", "load_workbook(径路)"),
    "save": ("Doc / Book", "d.save(径路) / b.save(径路)", "d.save(径路)", "wb.save(径路)"),
    "pdf": ("", "", "", ""),
    "copy": ("Paragraph / Cell", "p.text = 値 / s['A1'] = 値", "p.text = 値", "ws['A1'] = 値"),
    "cut": ("Paragraph / Cell", "p.text = 値 / s['A1'] = 値", "p.text = 値", "ws['A1'] = 値"),
    "paste": ("Paragraph / Cell", "p.text = 値 / s['A1'] = 値", "p.text = 値", "ws['A1'] = 値"),
    "clear": ("Run / Cell", "r.clear() / s['A1'] = None", "r.clear()", "ws['A1'] = None"),
    "bold": ("Run / Cell", "r.bold / c.font", "r.bold", "c.font = Font(bold=True)"),
    "italic": ("Run / Cell", "r.italic / c.font", "r.italic", "c.font = Font(italic=True)"),
    "underline": ("Run / Cell", "r.underline / c.font", "r.underline", "c.font = Font(underline=…)"),
    "strikeout": ("Run", "r.strike", "r.font.strike", ""),
    "fontname": ("Run / Cell", "r.font / c.font", "r.font.name", "c.font = Font(name=…)"),
    "fontsize": ("Run / Cell", "r.size_pt / c.font", "r.font.size", "c.font = Font(size=…)"),
    "incfont": ("Run / Cell", "r.size_pt / c.font", "r.font.size", "c.font = Font(size=…)"),
    "decfont": ("Run / Cell", "r.size_pt / c.font", "r.font.size", "c.font = Font(size=…)"),
    "fontcolor": ("Run / Cell", "r.color / c.font", "r.font.color.rgb", "c.font = Font(color=…)"),
    "superscript": ("Run", "", "r.font.superscript", ""),
    "subscript": ("Run", "", "r.font.subscript", "c.font = Font(vertAlign=…)"),
    "clearstyle": ("Run", "r.clear()", "r.clear()", ""),
    "ruby": ("Run", "", "", ""),
    "fillparag": ("Cell", "c.fill", "", "c.fill = PatternFill(…)"),
    "borders": ("Cell", "c.border", "", "c.border = Border(…)"),
    "align-left": ("Paragraph / Cell", "p.align / c.alignment", "p.alignment", "c.alignment = Alignment(…)"),
    "align-center": ("Paragraph / Cell", "p.align / c.alignment", "p.alignment", "c.alignment = Alignment(…)"),
    "align-right": ("Paragraph / Cell", "p.align / c.alignment", "p.alignment", "c.alignment = Alignment(…)"),
    "align-just": ("Paragraph", "p.align = 'justify'", "p.alignment", ""),
    "align-dist": ("Paragraph", "p.align = 'distribute'", "", ""),
    "wrap": ("Cell", "c.alignment", "", "c.alignment = Alignment(wrap_text=True)"),
    "merge": ("Cell", "(col_span / v_merge) / s.merge_cells(…)", "cell.merge(…)", "ws.merge_cells('A1:B2')"),
    "parastyle": ("Paragraph", "p.style", "p.style", ""),
    "markers": ("Paragraph", "p.style = '箇条書き'", "p.style = 'List Bullet'", ""),
    "numbering": ("Paragraph", "p.style = '番号付き'", "p.style = 'List Number'", ""),
    "multilevels": ("Paragraph", "", "", ""),
    "decoffset": ("Paragraph", "p.paragraph_format", "p.paragraph_format.left_indent", ""),
    "incoffset": ("Paragraph", "p.paragraph_format", "p.paragraph_format.left_indent", ""),
    "linespace": ("Paragraph", "p.paragraph_format.line_spacing", "p.paragraph_format.line_spacing", ""),
    "replace": ("Doc", "d.replace(前, 後)", "", ""),
    "format": ("Cell", "c.number_format", "", "c.number_format"),
    "currency": ("Cell", "c.number_format", "", "c.number_format"),
    "percents": ("Cell", "c.number_format", "", "c.number_format"),
    "comma": ("Cell", "c.number_format", "", "c.number_format"),
    "cell-ins": ("Table / Sheet", "t.add_row() / s.insert_rows(行)", "t.add_row()", "ws.insert_rows(行)"),
    "cell-del": ("Sheet", "s.delete_rows(行)", "", "ws.delete_rows(行)"),
    "condformat": ("Cell", "", "", "ws.conditional_formatting.add(…)"),
    "sum": ("Cell", "s['A1'] = '=SUM(…)'", "", "ws['A1'] = '=SUM(…)'"),
    "defname": ("Book", "b.create_named_range(名前, …)", "", "wb.defined_names"),
    "sort-asc": ("Sheet", "", "", ""),
    "sort-desc": ("Sheet", "", "", ""),
    "setfilter": ("Sheet", "", "", "ws.auto_filter.ref"),
    "clear-filter": ("Sheet", "", "", "ws.auto_filter"),
    "instable": ("Doc / Sheet", "d.add_table(行, 列) / s.add_table(…)", "d.add_table(行, 列)", "ws.add_table(…)"),
    "insimage": ("Doc", "d.add_picture(径路)", "d.add_picture(径路)", "ws.add_image(…)"),
    "inschart": ("Sheet", "", "", "ws.add_chart(…)"),
    "blankpage": ("Doc", "d.add_page_break()", "d.add_page_break()", ""),
    "pagebreak": ("Doc", "d.add_page_break()", "d.add_page_break()", "ws.row_breaks"),
    "edit-header": ("Doc / Sheet", "d.header / s.oddHeader", "section.header", "ws.oddHeader"),
    "edit-footer": ("Doc", "d.footer", "section.footer", ""),
    "controls": ("Doc(記入欄)", "d.fields()", "", ""),
    "insequation": ("Doc", "", "", ""),
    "inshyperlink": ("Cell", "c.hyperlink", "", "c.hyperlink"),
    "pivot-insert": ("Sheet", "", "", "ws.add_pivot(…)"),
    "pagemargins": ("Section", "d.sections[0]", "section.left_margin", "ws.page_margins"),
    "pageorient": ("Section", "d.sections[0]", "section.orientation", "ws.page_setup.orientation"),
    "pagesize": ("Section", "d.sections[0]", "section.page_width", "ws.page_setup.paperSize"),
    "printarea": ("Sheet", "s.print_area", "", "ws.print_area"),
    "printtitles": ("Sheet", "s.print_title_rows", "", "ws.print_title_rows"),
    "print-gridlines": ("Sheet", "s.print_gridlines", "", "ws.print_options.gridLines"),
    "insert-function": ("Cell", "s['A1'] = '=…'", "", "ws['A1'] = '=…'"),
    "calc-mode": ("Book", "b.recalc()", "", "wb.calculation"),
    "data-validation": ("Sheet", "s.add_data_validation(…)", "", "ws.add_data_validation(…)"),
    "group": ("Sheet", "s.row_groups", "", "ws.column_dimensions[…].outline_level"),
    "ungroup": ("Sheet", "s.row_groups", "", "ws.column_dimensions[…].outline_level"),
    "toc": ("Doc", "", "", ""),
    "bookmarks": ("Paragraph", "", "", ""),
    "crossref": ("Paragraph", "", "", ""),
    "footnote": ("Paragraph", "", "", ""),
    "caption": ("Paragraph", "", "", ""),
    "form-text": ("Doc(記入欄)", "d.fill(名前, 値)", "", ""),
    "form-name": ("Doc(記入欄)", "d.fields()", "", ""),
    "co-addcomment": ("Comment", "p.add_comment(文) / c.comment", "p.add_comment(文)", "c.comment = Comment(…)"),
    "co-showcomment": ("Comment", "d.comments / c.comment", "d.comments", "c.comment"),
    "prot-doc": ("Sheet", "", "", "ws.protection"),
    "prot-encrypt": ("Book", "", "", "wb.security"),
    "freeze": ("Sheet", "s.freeze_panes", "", "ws.freeze_panes"),
    "show-gridlines": ("Sheet", "s.show_gridlines", "", "ws.sheet_view.showGridLines"),
}

# **実装しないと決めた物**(id → 理由)。
# *ここに載せるのは、決めが記録されている物だけ*です。
# 決めていない空欄は「未実装」— 作らないと決めたのではなく、まだ作っていません。
TSUKURANAI = {
    # ファイルのページの、画面だけの物。**文書は変わりません**
    "f-back": "画面の行き来です。文書は変わりません",
    "f-recent": "画面が覚えている物です。プログラムは径路を直に書きます",
    "f-find": "画面の検索です。プログラムは自分でフォルダを歩けます",
    "f-place": "画面の操作です。プログラムは os が持っています",
    "f-quit": "画面の操作です",
    "f-opts": "アプリの設定です。文書は変わりません",
    "f-help": "画面の操作です",
    "f-req": "画面の操作です",
    "inschart": "図は matplotlib が描いて貼ります。見本で足ります(SEKKEI「見本を作って止める」)",
    "pivot-insert": "集計は polars が処理します。画面のボタンから使えます",
}


# **書けば済む物**(id → 書き方)。専用の口は作りません。
#
# 発注者 2026-08-24「別にマクロ等で書けたらいいので、すべて操作できるように
# するのは難しくない」。*そのとおりで、いまある口を組み合わせれば書ける物が
# たくさんあります*。専用の口を足すより、書き方を1行見せるほうが早いのです。
KAKEBA = {
    "changecase": "r.text = r.text.upper()",
    "inssymbol": "r.text += '※'(字をそのまま打つ)",
    "datetime": "p.text = date.today().strftime('%Y年%m月%d日')",
    "selectall": "d.paragraphs(全部を順に回る)",
    "text-from-file": "d.add_paragraph(open('メモ.txt').read())",
    "rem-duplicates": "見た = set() で行を選り分ける",
    "flash-fill": "s['B2'] = s['A2'].split()[0](規則を書く)",
    "text-column": "s['B2'], s['C2'] = s['A2'].split(',')",
    "subtotal": "s['A9'] = '=SUBTOTAL(9,A2:A8)'",
    "trace-prec": "s.formula('A1') で参照を読む",
    "show-formulas": "s.formula(場所)",
    "fill-num": "for i in range(10): s[f'A{i+2}'] = i + 1",
    "numpages": "(ヘッダーの `##`)",
    "pagenum": "(ヘッダーの `#`)",
    "insrecommend": "s.values() を polars に渡して選ぶ",
    "func-list": "自分の .py を書く(綴りの macros)",
    "csv-kind": "csv モジュールで読む",
    "data-from-text": "csv モジュールで読んで s['A1'] へ",
}


def 状態(id_: str, ow: str) -> str:
    """印を返す(2026-08-24 発注者「実装できたら ✅、実装しないは ❌」)。

    * `✅` 実装した — 専用の呼び方があります
    * `✍` 書けば済む — 専用の口は作りません。書き方をその行に出します
    * `❌` 実装しない — 決めた物だけ。理由をその行に出します
    * *空* まだ — 決めていない物。**ここが仕事の一覧**です
    """
    if ow:
        return "✅"
    if id_ in TSUKURANAI:
        return "❌"
    if id_ in KAKEBA:
        return "✍"
    return ""


# アイコンの置き場(この文書から見た相対の径路)
ICON_DIR = "../face/icons"

ICONS_RS = ROOT / "face/src/icons.rs"


def _icon_file() -> dict:
    """**絵の名前 → ファイル名。** `icons.rs` が繋いでいる対応を読みます。

    名前とファイル名が違う物があります(`insertimage` の実体は
    `insimage.svg`)。画面は `icons.rs` を通るので出ますが、文書から
    直に指すと届きません — 2026-08-24 にこの表で1件踏みました。
    """
    out = {}
    try:
        src = ICONS_RS.read_text(encoding="utf-8")
    except OSError:
        return out
    for m in re.finditer(r'\("([a-z0-9-]+)",\s*include_bytes!\("\.\./icons/([^"]+)\.svg"\)\)', src):
        if m.group(1) != m.group(2):
            out[m.group(1)] = m.group(2)
    return out


ICON_FILE = _icon_file()

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


FILE_SRC = ROOT / "writer/src/cmds.rs"

# ファイルのページの項目 → (オブジェクト, officework, python-docx, openpyxl)。
# **リボンのファイルタブは3つしかありません**(開く・保存・印刷)。
# 実体は全面のページで、`writer/src/cmds.rs` の `file_menu()` にあります。
# リボンだけを読むと、*ファイルの仕事がほとんど表に出ません*
# (2026-08-24 発注者「どうして対応表を変更しないのだ」)
FILE_MICHI = {
    "f-new": ("Doc / Book", "Doc() / Book()", "docx.Document()", "Workbook()"),
    "f-tpl": ("Template", "", "docx.Document(雛形)", "load_workbook(雛形)"),
    "f-open": ("Doc / Book", "Doc.open(径路) / Book.open(径路)", "docx.Document(径路)", "load_workbook(径路)"),
    "f-url": ("Doc", "", "", ""),
    "f-recent": ("", "", "", ""),
    "f-find": ("", "", "", ""),
    "f-recover": ("", "", "", ""),
    "f-save": ("Doc / Book", "d.save(径路) / b.save(径路)", "d.save(径路)", "wb.save(径路)"),
    "f-saveas": ("Doc / Book", "d.save(別の径路)", "d.save(別の径路)", "wb.save(別の径路)"),
    "f-print": ("Doc", "", "", ""),
    "f-merge": ("Doc", "d.render(値, rows=行)", "", ""),
    "f-html": ("Doc", "", "", ""),
    "f-protect": ("Doc / Book", "", "", "wb.security"),
    "f-distill": ("Doc", "", "", ""),
    "f-info": ("Doc / Book", "d.core_properties", "d.core_properties", "wb.properties"),
    "f-place": ("", "", "", ""),
    "f-quit": ("", "", "", ""),
    "f-opts": ("", "", "", ""),
    "f-help": ("", "", "", ""),
    "f-req": ("", "", "", ""),
    "f-back": ("", "", "", ""),
}


def file_menu():
    """ファイルのページの項目を `writer/src/cmds.rs` から読みます。手で写しません。"""
    src = FILE_SRC.read_text(encoding="utf-8")
    body = src[src.index("fn file_menu"):]
    body = body[: body.index("\n    }")]
    return re.findall(r'I::new\("(f-[a-z]+)",\s*ui::t!\("([^"]+)"\)\)', body)


def rows():
    """(段, ボタン, 絵, オブジェクト, 印, officework, python-docx, openpyxl)。
    **並びはメニューのまま**、*分類はオブジェクト*です(2026-08-24 発注者)。"""
    tabs = ribbon_parse.tables_or_die()
    並び = 段の並び(tabs)
    w = {t.name: t for t in tabs["WRITER"]}
    c = {t.name: t for t in tabs["CALC"]}
    out = []
    for 段 in 並び:
        if 段 == "ファイル":
            # **リボンの3つではなく、全面のページの一覧を出します**
            for i, ラベル in file_menu():
                if i not in FILE_MICHI:
                    continue
                obj, ow, pd, op = FILE_MICHI[i]
                _ラベルの逆引き[i] = ラベル
                out.append((段, ラベル, "", obj, 状態(i, ow), ow, pd, op))
            continue
        見た = set()
        for t in (w.get(段), c.get(段)):
            if t is None:
                continue
            for cmd in t.cmds:
                if not cmd.id or cmd.id in 見た or cmd.id not in MICHI:
                    continue
                見た.add(cmd.id)
                obj, ow, pd, op = MICHI[cmd.id]
                _ラベルの逆引き[cmd.id] = cmd.label
                out.append((段, cmd.label, cmd.icon, obj, 状態(cmd.id, ow), ow, pd, op))
    return out


_ラベルの逆引き: dict = {}


def 理由(ラベル: str, st: str):
    """「実装しない」の理由。表の中で読めるようにします"""
    if st not in ("❌", "✍"):
        return None
    for 表 in (TSUKURANAI, KAKEBA):
        for k, v in 表.items():
            if _ラベルの逆引き.get(k) == ラベル:
                return v
    return None


def 表() -> str:
    r = rows()
    o = []
    o.append("1行が1つのボタンです。*並びは画面のメニューのまま*です。\n")
    o.append("*`officework` は `.adoc` を触る1つの模型*なので、文書と表で分けません。")
    o.append("代わりに**どのオブジェクトの物か**を出します。")
    o.append("`A / B` と2つ書いてある所は、*いま呼び方が2つある*という意味です。\n")
    o.append("空いている所(—)は、その道がありません。\n")
    o.append("この節は `tools/api_taiou.py` が起こします。手で直さないでください。\n")
    いま = None
    for 段, ラベル, 絵, obj, st, ow, pd, op in r:
        if 段 != いま:
            if いま is not None:
                o.append("|===\n")
            # **見出しは `==`。** `===` にすると本家が「段が飛んでいる」と
            # 警告します(この節の前に `==` が無いため。2026-08-24 に実際に出た)
            o.append(f"== {段}")
            o.append("")
            o.append('[cols="2,2,^1,3,3,3"]')
            o.append("|===")
            o.append("|ボタン |オブジェクト |印 |officework |python-docx |openpyxl\n")
            いま = 段
        f = lambda x: x if x else "—"
        中 = ow if ow else (理由(ラベル, st) or "—")
        # **絵を名前の前に出します**(2026-08-24 発注者)。画面で見ている物と
        # 同じ絵なので、名前より先に目に入ります。径路は `face/icons` から
        # この文書の場所への相対です
        # **絵の名前とファイル名は、同じとは限りません。**
        # `face/src/icons.rs` が名前とファイルを繋いでいます(例: `insertimage`
        # の実体は `insimage.svg`)。画面はそちらを通るので出ますが、
        # 文書から直に指すと届きません。ここで解いてから書きます
        名 = ICON_FILE.get(絵, 絵)
        絵札 = f"image:{ICON_DIR}/{名}.svg[{ラベル},16,16] " if 名 else ""
        o.append(f"|{絵札}{ラベル} |{f(obj)} |{st} |{中} |{f(pd)} |{f(op)}")
    if いま is not None:
        o.append("|===\n")
    return "\n".join(o)


def 覆い():
    """**この表がどれだけ覆っているか**(2026-08-24)。

    「Python ですべて操作できる」と言うには、*表が全部のボタンを載せている*
    必要があります。載っていないボタンは、状態すら分かりません。
    """
    tabs = ribbon_parse.tables_or_die()
    全 = {}
    for app in ("WRITER", "CALC"):
        for tab in tabs[app]:
            for c in tab.cmds:
                if c.id:
                    全.setdefault(c.id, (tab.name, c.label))
    # **ファイルのページも数えます**(リボンのファイルタブは3つだけで、
    # 実際の仕事は全面のページにあります)
    for i, ラベル in file_menu():
        全.setdefault(i, ("ファイル", ラベル))
    のせた = [k for k in 全 if k in MICHI or k in FILE_MICHI]
    return len(のせた), len(全), sorted(
        (v[0], v[1], k) for k, v in 全.items() if k not in MICHI and k not in FILE_MICHI
    )


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
    のせた, 全, 抜け = 覆い()
    if "--todo" in sys.argv:
        print(f"対応表に載っていないボタン {len(抜け)} 種:")
        for 段, l, i in 抜け:
            print(f"  {段:<12} {l:<24} {i}")
        return 0
    print(f"対応表は実物と揃っています({len(rows())} 行)。"
          f"押せるボタン {全} 種のうち {のせた} 種を載せています"
          f"(--todo で残りが出ます)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
