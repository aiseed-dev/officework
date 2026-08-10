"""再計算を実機で見るための帳面を1枚、xlsx を直に組んで作る。

**依存を3段にしてある。** 1段だけだと「打った字が入った」のか
「本当に計算し直した」のか区別がつかない。B2 を打ち替えて B6 まで
追随すれば、依存の伝播が効いている。

**式には古い答えを控えてある**(`<v>`)。再計算が走らなければ、
画面には古い数字が残るので、走らなかったことがすぐ分かる。
とくに B7 の IF は 1200→1500 で「安い」から「高い」へ**裏返る**。

openpyxl を入れずに済ませるため XML を直に書いている。共有文字列を
使うのは、それが普通の道だから(inlineStr で読み手の別の枝を踏むと、
何を試しているのか分からなくなる)。
"""
import pathlib
import zipfile

NS = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"

shared: list[str] = []


def si(text: str) -> int:
    if text not in shared:
        shared.append(text)
    return shared.index(text)


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def cell(ref: str, *, s: int = 0, num=None, text=None, formula=None, cached=None) -> str:
    """1つのセル。text は共有文字列、formula は控えの答え付き。"""
    a = f'r="{ref}"' + (f' s="{s}"' if s else "")
    if formula is not None:
        t = ' t="str"' if isinstance(cached, str) else ""
        v = f"<v>{esc(str(cached))}</v>" if cached is not None else ""
        return f"<c {a}{t}><f>{esc(formula)}</f>{v}</c>"
    if text is not None:
        return f'<c {a} t="s"><v>{si(text)}</v></c>'
    if num is not None:
        return f"<c {a}><v>{num}</v></c>"
    return f"<c {a}/>"


BOLD, NUM, BOLDNUM = 1, 2, 3

rows1 = [
    (1, [cell("A1", s=BOLD, text="項目"), cell("B1", s=BOLD, text="値"),
         cell("C1", s=BOLD, text="検算"), cell("D1", s=BOLD, text="備考")]),
    (2, [cell("A2", text="単価"), cell("B2", s=NUM, num=1200),
         cell("D2", text="ここを打ち替える")]),
    (3, [cell("A3", text="数量"), cell("B3", num=3),
         cell("D3", text="ここも")]),
    (4, [cell("A4", text="小計"), cell("B4", s=NUM, formula="B2*B3", cached=3600),
         cell("C4", s=NUM, formula="B4", cached=3600),
         cell("D4", text="1段目 — 直接の依存")]),
    (5, [cell("A5", text="消費税(10%)"),
         cell("B5", s=NUM, formula="ROUND(B4*0.1,0)", cached=360),
         cell("C5", s=NUM, formula="B5", cached=360),
         cell("D5", text="2段目 — 依存の依存")]),
    (6, [cell("A6", s=BOLD, text="合計"),
         cell("B6", s=BOLDNUM, formula="B4+B5", cached=3960),
         cell("C6", s=NUM, formula="B6", cached=3960),
         cell("D6", text="3段目 — ここまで追随すれば本物")]),
    (7, [cell("A7", text="判定"),
         cell("B7", formula='IF(B6>=4000,"高い","安い")', cached="安い"),
         cell("D7", text="分岐も動くか — 1500 に打ち替えると裏返る")]),
    (8, [cell("A8", text="桁区切り"),
         cell("B8", formula='TEXT(B6,"#,##0")', cached="3,960"),
         cell("D8", text="文字を返す式")]),
    (9, [cell("A9", text="平均"),
         cell("B9", formula="AVERAGE(B2:B3)", cached=601.5),
         cell("D9", text="範囲の集計")]),
]

rows2 = [
    (1, [cell("A1", s=BOLD, text="シートを跨ぐ参照")]),
    (2, [cell("A2", text="合計を引く"),
         cell("B2", s=NUM, formula="再計算!B6", cached=3960)]),
    (3, [cell("A3", text="単価+数量"),
         cell("B3", s=NUM, formula="SUM(再計算!B2:B3)", cached=1203)]),
]


def sheet_xml(rows, cols, dim):
    body = "".join(f'<row r="{r}">{"".join(cs)}</row>' for r, cs in rows)
    return (
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<worksheet xmlns="{NS}"><dimension ref="{dim}"/>'
        f'<sheetViews><sheetView workbookViewId="0"/></sheetViews>'
        f'<sheetFormatPr defaultRowHeight="15"/>{cols}'
        f"<sheetData>{body}</sheetData></worksheet>"
    )


COLS1 = ('<cols><col min="1" max="1" width="22" customWidth="1"/>'
         '<col min="2" max="4" width="16" customWidth="1"/></cols>')
COLS2 = '<cols><col min="1" max="2" width="22" customWidth="1"/></cols>'

s1 = sheet_xml(rows1, COLS1, "A1:D9")
s2 = sheet_xml(rows2, COLS2, "A1:B3")

sst = (
    f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<sst xmlns="{NS}" count="{len(shared)}" uniqueCount="{len(shared)}">'
    + "".join(f"<si><t>{esc(t)}</t></si>" for t in shared)
    + "</sst>"
)

styles = (
    f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<styleSheet xmlns="{NS}">'
    f'<fonts count="2"><font><sz val="11"/><name val="Calibri"/></font>'
    f'<font><b/><sz val="11"/><name val="Calibri"/></font></fonts>'
    f'<fills count="2"><fill><patternFill patternType="none"/></fill>'
    f'<fill><patternFill patternType="gray125"/></fill></fills>'
    f'<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>'
    f'<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>'
    f'<cellXfs count="4">'
    f'<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>'
    f'<xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>'
    f'<xf numFmtId="3" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>'
    f'<xf numFmtId="3" fontId="1" fillId="0" borderId="0" xfId="0" applyNumberFormat="1" applyFont="1"/>'
    f"</cellXfs></styleSheet>"
)

workbook = (
    f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<workbook xmlns="{NS}" xmlns:r="{R}"><sheets>'
    f'<sheet name="再計算" sheetId="1" r:id="rId1"/>'
    f'<sheet name="別シート" sheetId="2" r:id="rId2"/>'
    f"</sheets></workbook>"
)

wb_rels = (
    f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
    f'<Relationship Id="rId1" Type="{R}/worksheet" Target="worksheets/sheet1.xml"/>'
    f'<Relationship Id="rId2" Type="{R}/worksheet" Target="worksheets/sheet2.xml"/>'
    f'<Relationship Id="rId3" Type="{R}/styles" Target="styles.xml"/>'
    f'<Relationship Id="rId4" Type="{R}/sharedStrings" Target="sharedStrings.xml"/>'
    f"</Relationships>"
)

root_rels = (
    f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
    f'<Relationship Id="rId1" Type="{R}/officeDocument" Target="xl/workbook.xml"/>'
    f"</Relationships>"
)

CT = "http://schemas.openxmlformats.org/officeDocument/2006/"
content_types = (
    f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
    f'<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
    f'<Default Extension="xml" ContentType="application/xml"/>'
    f'<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
    f'<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
    f'<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
    f'<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>'
    f'<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>'
    f"</Types>"
)

out = pathlib.Path.home() / "xlsx-corpus" / "recalc_test.xlsx"
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", content_types)
    z.writestr("_rels/.rels", root_rels)
    z.writestr("xl/workbook.xml", workbook)
    z.writestr("xl/_rels/workbook.xml.rels", wb_rels)
    z.writestr("xl/worksheets/sheet1.xml", s1)
    z.writestr("xl/worksheets/sheet2.xml", s2)
    z.writestr("xl/styles.xml", styles)
    z.writestr("xl/sharedStrings.xml", sst)
print(out)
