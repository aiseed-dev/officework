"""**絵の入った xlsx を作る。** 手元に1枚も無かったので。

corpus 26 枚は官庁の統計表ばかりで、`xl/media/` はすべて空だった。同梱の
見本・型紙9枚も0。**「絵は実物に出ない」のではなく、この26枚では答えが
出ない** — 統計表にロゴを入れる人はいない(2026-08-10)。

商用の帳票(見積書・請求書・報告書)にはロゴが入る。genoffice を使う人が
最初に開くのはそちらなので、**絵を一度も開かずに絵の実装を始めない**ために
型紙を作る。

2種類を出す:

    media_hand.xlsx  ここで手で組んだ物。**アンカーの型を全部入れてある**
                     (twoCell / oneCell / 図形)
    media_lo.xlsx    それを LibreOffice に焼き直させた物。**実物の道具が
                     何を書くか**は、こちらでないと分からない

依存を増やさない。PNG も zlib と標準ライブラリだけで組む。
"""

from __future__ import annotations

import pathlib
import struct
import subprocess
import sys
import zlib
import zipfile

NS_MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
NS_R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
NS_XDR = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
NS_A = "http://schemas.openxmlformats.org/drawingml/2006/main"
PKG = "http://schemas.openxmlformats.org/package/2006/relationships"

EMU_PER_PX = 9525

# **最小のグラフの部品。** 中身は読まないので、形が整っていれば足りる
CHART_XML = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">'
    "<c:chart><c:plotArea><c:barChart/></c:plotArea></c:chart></c:chartSpace>"
)


def png(width: int, height: int, rgb: tuple[int, int, int]) -> bytes:
    """べた塗りの PNG を1枚。**外の道具を使わない**ので、型紙が誰の手元でも同じ。"""

    def chunk(tag: bytes, body: bytes) -> bytes:
        return (
            struct.pack(">I", len(body))
            + tag
            + body
            + struct.pack(">I", zlib.crc32(tag + body) & 0xFFFFFFFF)
        )

    raw = b"".join(b"\x00" + bytes(rgb) * width for _ in range(height))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def anchor_from_to(c1, r1, c2, r2, off=76200) -> str:
    """`xdr:twoCellAnchor` — **左上と右下の両方をセルに留める**。表と一緒に
    伸び縮みする置き方で、帳票のロゴはたいていこれ。"""
    return (
        f'<xdr:twoCellAnchor editAs="oneCell">'
        f"<xdr:from><xdr:col>{c1}</xdr:col><xdr:colOff>{off}</xdr:colOff>"
        f"<xdr:row>{r1}</xdr:row><xdr:rowOff>{off}</xdr:rowOff></xdr:from>"
        f"<xdr:to><xdr:col>{c2}</xdr:col><xdr:colOff>0</xdr:colOff>"
        f"<xdr:row>{r2}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>"
    )


def anchor_one(c1, r1, w_px, h_px) -> str:
    """`xdr:oneCellAnchor` — 左上だけ留めて大きさは固定。**`to` が無い**ので、
    ここを読み落とすと絵の位置が出ない。"""
    return (
        f"<xdr:oneCellAnchor>"
        f"<xdr:from><xdr:col>{c1}</xdr:col><xdr:colOff>0</xdr:colOff>"
        f"<xdr:row>{r1}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>"
        f'<xdr:ext cx="{w_px * EMU_PER_PX}" cy="{h_px * EMU_PER_PX}"/>'
    )


def pic(id_: int, name: str, rid: str) -> str:
    return (
        f"<xdr:pic><xdr:nvPicPr>"
        f'<xdr:cNvPr id="{id_}" name="{name}"/><xdr:cNvPicPr/></xdr:nvPicPr>'
        f'<xdr:blipFill><a:blip xmlns:r="{NS_R}" r:embed="{rid}"/>'
        f"<a:stretch><a:fillRect/></a:stretch></xdr:blipFill>"
        f"<xdr:spPr><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr>"
        f"</xdr:pic><xdr:clientData/>"
    )


def chart_frame(id_: int, name: str, rid: str) -> str:
    """`xdr:graphicFrame` — グラフの入れ物。

    **officework はグラフの模型を持たない**(描くのは matplotlib)。だから
    読み手は中に入らず、**在ったことだけを帳簿に載せる**。その道が働くかを
    確かめるために、型紙に1つ入れておく(2026-08-11)。
    """
    return (
        f"<xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr>"
        f'<xdr:cNvPr id="{id_}" name="{name}"/><xdr:cNvGraphicFramePr/>'
        f"</xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/>"
        f'<a:ext cx="4000000" cy="2500000"/></xdr:xfrm>'
        f'<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">'
        f'<c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" '
        f'xmlns:r="{NS_R}" r:id="{rid}"/></a:graphicData></a:graphic>'
        f"</xdr:graphicFrame><xdr:clientData/>"
    )


def shape(id_: int, name: str, text: str, color: str) -> str:
    """`xdr:sp` — 絵ではない図形。**kind が image ではなく shape** で来る。"""
    return (
        f"<xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr>"
        f'<xdr:cNvPr id="{id_}" name="{name}"/><xdr:cNvSpPr/></xdr:nvSpPr>'
        f'<xdr:spPr><a:prstGeom prst="roundRect"><a:avLst/></a:prstGeom>'
        f'<a:solidFill><a:srgbClr val="{color}"/></a:solidFill></xdr:spPr>'
        f"<xdr:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{text}</a:t></a:r></a:p>"
        f"</xdr:txBody></xdr:sp><xdr:clientData/>"
    )


def build(out: pathlib.Path) -> None:
    logo = png(120, 60, (0x1F, 0x6F, 0xB2))
    stamp = png(64, 64, (0xC0, 0x30, 0x30))

    rows = [
        ("御見積書", "", "", ""),
        ("", "", "", ""),
        ("", "", "", ""),
        ("品名", "数量", "単価", "金額"),
        ("天井材 A-1", 12, 3800, "=B5*C5"),
        ("壁材 W-2", 30, 1250, "=B6*C6"),
        ("", "", "合計", "=SUM(D5:D6)"),
    ]
    cells = []
    for r, row in enumerate(rows, start=1):
        cs = []
        for c, v in enumerate(row):
            if v == "":
                continue
            ref = f"{chr(65 + c)}{r}"
            if isinstance(v, str) and v.startswith("="):
                cs.append(f'<c r="{ref}"><f>{v[1:]}</f><v>0</v></c>')
            elif isinstance(v, (int, float)):
                cs.append(f'<c r="{ref}"><v>{v}</v></c>')
            else:
                cs.append(f'<c r="{ref}" t="inlineStr"><is><t>{v}</t></is></c>')
        if cs:
            cells.append(f'<row r="{r}">{"".join(cs)}</row>')

    sheet = (
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<worksheet xmlns="{NS_MAIN}" xmlns:r="{NS_R}">'
        f'<dimension ref="A1:D7"/>'
        f'<sheetViews><sheetView workbookViewId="0"/></sheetViews>'
        f'<sheetFormatPr defaultRowHeight="15"/>'
        f'<cols><col min="1" max="1" width="24" customWidth="1"/>'
        f'<col min="2" max="4" width="12" customWidth="1"/></cols>'
        f'<sheetData>{"".join(cells)}</sheetData>'
        f'<drawing r:id="rId1"/></worksheet>'
    )

    drawing = (
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<xdr:wsDr xmlns:xdr="{NS_XDR}" xmlns:a="{NS_A}">'
        # 0番目: 会社のロゴ。表と一緒に伸び縮みする置き方
        f'{anchor_from_to(0, 0, 2, 3)}{pic(2, "logo", "rId1")}</xdr:twoCellAnchor>'
        # 1番目: 検印。大きさ固定
        f'{anchor_one(3, 4, 64, 64)}{pic(3, "stamp", "rId2")}</xdr:oneCellAnchor>'
        # 2番目: 絵ではない図形
        f'{anchor_from_to(0, 8, 2, 10)}{shape(4, "caution", "confidential", "FFF2CC")}'
        f"</xdr:twoCellAnchor>"
        # 3番目: グラフ。**中身は持たないが、在ったことは言う**の道を試す
        f'{anchor_from_to(4, 0, 8, 8)}{chart_frame(5, "quarterly_sales", "rId3")}'
        f"</xdr:twoCellAnchor></xdr:wsDr>"
    )

    parts = {
        "[Content_Types].xml": (
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            f'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
            f'<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
            f'<Default Extension="xml" ContentType="application/xml"/>'
            f'<Default Extension="png" ContentType="image/png"/>'
            f'<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
            f'<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
            f'<Override PartName="/xl/drawings/drawing1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/>'
            f"</Types>"
        ),
        "_rels/.rels": (
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            f'<Relationships xmlns="{PKG}">'
            f'<Relationship Id="rId1" Type="{NS_R}/officeDocument" Target="xl/workbook.xml"/>'
            f"</Relationships>"
        ),
        "xl/workbook.xml": (
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            f'<workbook xmlns="{NS_MAIN}" xmlns:r="{NS_R}">'
            f'<sheets><sheet name="見積" sheetId="1" r:id="rId1"/></sheets></workbook>'
        ),
        "xl/_rels/workbook.xml.rels": (
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            f'<Relationships xmlns="{PKG}">'
            f'<Relationship Id="rId1" Type="{NS_R}/worksheet" Target="worksheets/sheet1.xml"/>'
            f"</Relationships>"
        ),
        "xl/worksheets/sheet1.xml": sheet,
        "xl/worksheets/_rels/sheet1.xml.rels": (
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            f'<Relationships xmlns="{PKG}">'
            f'<Relationship Id="rId1" Type="{NS_R}/drawing" Target="../drawings/drawing1.xml"/>'
            f"</Relationships>"
        ),
        "xl/drawings/drawing1.xml": drawing,
        "xl/drawings/_rels/drawing1.xml.rels": (
            f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            f'<Relationships xmlns="{PKG}">'
            f'<Relationship Id="rId1" Type="{NS_R}/image" Target="../media/image1.png"/>'
            f'<Relationship Id="rId2" Type="{NS_R}/image" Target="../media/image2.png"/>'
            f'<Relationship Id="rId3" Type="{NS_R}/chart" Target="../charts/chart1.xml"/>'
            f"</Relationships>"
        ),
    }

    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for name, body in parts.items():
            z.writestr(name, body)
        z.writestr("xl/charts/chart1.xml", CHART_XML)
        z.writestr("xl/media/image1.png", logo)
        z.writestr("xl/media/image2.png", stamp)


def rebake(src: pathlib.Path, out_dir: pathlib.Path) -> pathlib.Path | None:
    """LibreOffice に焼き直させる。**実物の道具が何を書くかは、こちらでしか
    分からない** — 手で組んだ型紙は、自分の思い込みしか映さない。"""
    r = subprocess.run(
        ["soffice", "--headless", "--convert-to", "xlsx", "--outdir", str(out_dir), str(src)],
        capture_output=True,
        timeout=180,
    )
    made = out_dir / f"{src.stem}.xlsx"
    if r.returncode != 0 or not made.exists():
        print(f"  LibreOffice が焼けませんでした: {r.stderr.decode()[:200]}", file=sys.stderr)
        return None
    return made


if __name__ == "__main__":
    dest = pathlib.Path.home() / "xlsx-corpus"
    dest.mkdir(exist_ok=True)
    hand = dest / "media_hand.xlsx"
    build(hand)
    print(f"手で組んだ: {hand}")

    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        made = rebake(hand, pathlib.Path(tmp))
        if made:
            lo = dest / "media_lo.xlsx"
            lo.write_bytes(made.read_bytes())
            print(f"LibreOffice が焼き直した: {lo}")

    for p in (hand, dest / "media_lo.xlsx"):
        if not p.exists():
            continue
        with zipfile.ZipFile(p) as z:
            n = z.namelist()
            print(f"  {p.name}: media {[x for x in n if 'media/' in x]} "
                  f"drawing {[x for x in n if x.startswith('xl/drawings/')]}")
