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
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).parent))
import ribbon_parse  # noqa: E402

# 操作 → (officework 文書, officework 表, python-docx, openpyxl)
# 空文字は「その道が無い」。**セルは writer にもあります**(2026-08-24 発注者)ので、
# 字・書式・結合・式は両方に道があります
KYOUTSUU = [
    ("開く", "doc.Doc.open(径路)", "sheet.Book.open(径路)", "docx.Document(径路)", "load_workbook(径路)"),
    ("保存", "d.save(径路)", "b.save(径路)", "d.save(径路)", "wb.save(径路)"),
    ("字を入れる", "p.text = 値", "s['A1'] = 値", "p.text = 値", "ws['A1'] = 値"),
    ("字を消す", "r.clear()", "s['A1'] = None", "r.clear()", "ws['A1'] = None"),
    ("太字", "r.bold", "c.font", "r.bold", "c.font = Font(bold=True)"),
    ("斜体", "r.italic", "c.font", "r.italic", "c.font = Font(italic=True)"),
    ("下線", "r.underline", "c.font", "r.underline", "c.font = Font(underline=…)"),
    ("書体", "r.font", "c.font", "r.font.name", "c.font = Font(name=…)"),
    ("大きさ", "r.size_pt", "c.font", "r.font.size", "c.font = Font(size=…)"),
    ("文字の色", "r.color", "c.font", "r.font.color.rgb", "c.font = Font(color=…)"),
    ("揃え", "p.align", "c.alignment", "p.alignment", "c.alignment = Alignment(…)"),
    ("セルの結合", "(表の col_span / v_merge)", "s.merge_cells('A1:B2')", "cell.merge(…)", "ws.merge_cells('A1:B2')"),
    ("表を作る", "d.add_table(行, 列)", "s.add_table(…)", "d.add_table(行, 列)", "ws.add_table(…)"),
    ("行を足す・抜く", "t.add_row()", "s.insert_rows(行)", "t.add_row()", "ws.insert_rows(行)"),
    ("式", "(表のセルに `=…`)", "s['A1'] = '=SUM(…)'", "", "ws['A1'] = '=SUM(…)'"),
    ("置き換え", "d.replace(前, 後)", "", "", ""),
    ("コメント", "p.add_comment(文)", "c.comment", "p.add_comment(文)", "c.comment = Comment(…)"),
    ("画像を入れる", "d.add_picture(径路)", "", "d.add_picture(径路)", "ws.add_image(…)"),
    ("ヘッダーとフッター", "d.header / d.footer", "s.oddHeader", "section.header", "ws.oddHeader"),
    ("用紙・余白・向き", "d.sections[0]", "", "section.page_width", "ws.page_setup"),
]

# 文書だけの操作(表には無い)
BUNSHO = [
    ("段落のスタイル", "p.style", "p.style"),
    ("箇条書き", "p.style = '箇条書き'", "p.style = 'List Bullet'"),
    ("行間・字下げ", "p.paragraph_format", "p.paragraph_format"),
    ("ルビ", "", ""),
    ("上付き・下付き", "", "r.font.superscript"),
    ("脚注", "", ""),
    ("しおり・相互参照", "", ""),
    ("目次", "", ""),
    ("記入欄", "d.fields() / d.fill(名前, 値)", ""),
    ("改ページ", "d.add_page_break()", "d.add_page_break()"),
]

# **共通にできるが、いまは表の側にしかない**(2026-08-24 発注者
# 「adoc に入れられるので、文書にもいれられます」)。
# 表の見た目は*名前*で adoc に入り、定義はテンプレートが持ちます
# (SEKKEI「セルのスタイルを名前で持つ」)。集計の道具は adoc の属性で持てます。
# *ここが仕事の一覧*です — 文書の側の列が埋まった日に、上の共通へ移します
MADA = [
    ("表示形式", "c.number_format", "c.number_format"),
    ("塗りつぶし", "c.fill", "c.fill = PatternFill(…)"),
    ("罫線", "c.border", "c.border = Border(…)"),
    ("条件付き書式", "", "ws.conditional_formatting.add(…)"),
    ("入力規則", "s.add_data_validation(…)", "ws.add_data_validation(…)"),
    ("フィルター", "", "ws.auto_filter.ref"),
    ("グループ化", "s.row_groups", "ws.column_dimensions[…].outline_level"),
    ("名前の定義", "b.create_named_range(名前, …)", "wb.defined_names"),
    ("ピボットテーブル", "", "ws.add_pivot(…)"),
    ("グラフ", "", "ws.add_chart(…)"),
]

# 表だけの操作。**ここが本当に表の専用** — 紙に出す範囲と、画面の見え方
HYOU = [
    ("印刷範囲", "s.print_area", "ws.print_area"),
    ("タイトル行の繰り返し", "s.print_title_rows", "ws.print_title_rows"),
    ("枠線も印刷", "s.print_gridlines", "ws.print_options.gridLines"),
    ("ウィンドウ枠の固定", "s.freeze_panes", "ws.freeze_panes"),
    ("枠線表示", "s.show_gridlines", "ws.sheet_view.showGridLines"),
    ("計算方法", "b.recalc()", "wb.calculation"),
    ("保護", "", "ws.protection / wb.security"),
]

MARK_S = "// api:taiou:start"
MARK_E = "// api:taiou:end"
SAKI = ROOT / "docs/api-taiou.ja.adoc"


def rows():
    """(群, 操作, 文書, 表, python-docx, openpyxl)"""
    out = [("共通", 操作, 文, 表, pd, op) for 操作, 文, 表, pd, op in KYOUTSUU]
    out += [("文書だけ", 操作, 文, "—", pd, "—") for 操作, 文, pd in BUNSHO]
    out += [("まだ表だけ", 操作, "—", 表, "—", op) for 操作, 表, op in MADA]
    out += [("表だけ", 操作, "—", 表, "—", op) for 操作, 表, op in HYOU]
    return out


def 表() -> str:
    o = []
    o.append("1行が1つの操作です。`officework` の呼び方と、本家の呼び方が横に並びます。")
    o.append("空いている所(—)は、その道が無いという意味です。\n")
    o.append("この節は `tools/api_taiou.py` が起こします。手で直さないでください。\n")
    群 = [
        ("共通", "文書でも表でもできること",
         "*セルは文書の表にもあります*(2026-08-24 発注者)。"
         "字・書式・結合・式は、どちらでも同じようにできます。"),
        ("文書だけ", "文書にしかないこと", "段落と紙面の話です。"),
        ("まだ表だけ", "共通にできるが、いま表の側にしかないこと",
         "*どれも AsciiDoc に書けるので、文書の表にも入れられます*"
         "(2026-08-24 発注者)。見た目は**名前**で入り、定義はテンプレートが持ちます"
         "(SEKKEI「セルのスタイルを名前で持つ」)。集計の道具は属性で持てます。"
         "**ここが仕事の一覧です** — 文書の列が埋まった日に、上の「共通」へ移します。"),
        ("表だけ", "表にしかないこと",
         "*ここが本当に表の専用*です。紙に出す範囲と、画面の見え方だけが残りました。"),
    ]
    r = rows()
    for 名, 見出し, 説明 in 群:
        o.append(f"=== {見出し}")
        o.append("")
        o.append(説明)
        o.append("")
        # **本家の2つを横に並べます**(2026-08-24 発注者)。
        # 文書と表で officework の呼び方が違うので、その2つを先に置き、
        # python-docx と openpyxl を隣り合わせにします — *同じ操作を
        # 本家がどう呼ぶか*を、左右で見比べられる形です
        # **その群に無い側の列は出しません。** 「表だけ」の群で文書の2列を
        # 出すと、全部が「—」で埋まります。読む人には邪魔なだけです
        文書側 = any(g == 名 and (文 != "—" or pd != "—") for g, _, 文, _, pd, _ in r)
        表側 = any(g == 名 and (表c != "—" or op != "—") for g, _, _, 表c, _, op in r)
        f = lambda x: x if x else "—"
        if 文書側 and not 表側:
            o.append('[cols="2,3,3"]')
            o.append("|===")
            o.append("|操作 |officework(文書) |python-docx\n")
            for g, 操作, 文, 表c, pd, op in r:
                if g == 名:
                    o.append(f"|{操作} |{f(文)} |{f(pd)}")
        elif 文書側:
            o.append('[cols="2,3,3,3,3"]')
            o.append("|===")
            o.append("|操作 |officework(文書) |python-docx |officework(表) |openpyxl\n")
            for g, 操作, 文, 表c, pd, op in r:
                if g == 名:
                    o.append(f"|{操作} |{f(文)} |{f(pd)} |{f(表c)} |{f(op)}")
        else:
            o.append('[cols="2,3,3"]')
            o.append("|===")
            o.append("|操作 |officework(表) |openpyxl\n")
            for g, 操作, 文, 表c, pd, op in r:
                if g == 名:
                    o.append(f"|{操作} |{f(表c)} |{f(op)}")
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
        print(f"::error::{SAKI.name} の対応表が実物とずれています"
              "(python3 tools/api_taiou.py --write で直します)", file=sys.stderr)
        return 1
    print(f"対応表は実物と揃っています({len(rows())} 行)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
