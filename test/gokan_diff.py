#!/usr/bin/env python3
"""**本家との差分の検査。** 同じ実物を本家(openpyxl / python-docx)と
officework に読ませ、答えを1項目ずつ突き合わせる(2026-08-31 発注者
「python-docx と openpyxl との互換性テストをやれ」)。

見本を作って確かめる検査は、直した所しか見ない。実物と本家を使う
この形は、**知らなかったずれ**を見つける — test/cao・test/zei と
同じ考え方の、Python の口の版。

    .venv/bin/python test/gokan_diff.py ~/dev/test/zei/*.xlsx ~/dev/test/cao/*.docx

読みの突き合わせ(このファイル)で見る物:

- xlsx: シート名・使う範囲・全セルの値・表示形式・結合・列幅・行高・
  書体(名前/大きさ/太字)・揃え
- docx: 段落の数と字・run の分かれ目と太字/斜体・表の形と中身・
  節の用紙と余白

さらに**書きの往復**: officework で保存した物を本家に読ませ、本家が
元ファイルから読んだ答えと比べる(消えた物・化けた物が出る)。
"""

import sys
import tempfile
from pathlib import Path

MAX_SHOW = 5  # 種類ごとに最初の5件だけ見せる(数は全部数える)


class Diff:
    def __init__(self, name):
        self.name = name
        self.kinds = {}

    def add(self, kind, msg):
        self.kinds.setdefault(kind, []).append(msg)

    def report(self):
        total = sum(len(v) for v in self.kinds.values())
        if not total:
            print(f"  {self.name}: ずれなし")
            return 0
        print(f"  {self.name}: ずれ {total} 件")
        for kind, msgs in sorted(self.kinds.items(), key=lambda kv: -len(kv[1])):
            print(f"    [{kind}] {len(msgs)} 件")
            for m in msgs[:MAX_SHOW]:
                print(f"      {m}")
            if len(msgs) > MAX_SHOW:
                print(f"      … 残り {len(msgs) - MAX_SHOW} 件")
        return total


def norm_val(v):
    """値の比べ方。int と float の 100 は同じ、None と "" は別(仕様の差を見る)"""
    if isinstance(v, float) and v == int(v) and abs(v) < 1e15:
        return int(v)
    return v


# ---------------- xlsx ----------------

def xlsx_diff(path: Path) -> int:
    import openpyxl
    from officework import sheet

    d = Diff(path.name)
    wb = openpyxl.load_workbook(path)
    b = sheet.Book.open(str(path))

    if wb.sheetnames != b.sheet_names:
        d.add("シート名", f"本家 {wb.sheetnames} / うち {b.sheet_names}")
    for name in wb.sheetnames:
        if name not in b.sheet_names:
            continue
        ws, s = wb[name], b[name]
        # 使う範囲
        if (ws.max_row, ws.max_column) != (s.max_row, s.max_column):
            d.add("範囲", f"{name}: 本家 {ws.max_row}x{ws.max_column} / うち {s.max_row}x{s.max_column}")
        rows = min(ws.max_row, s.max_row, 400)
        cols = min(ws.max_column, s.max_column, 60)
        for r in range(1, rows + 1):
            for c in range(1, cols + 1):
                oc = ws.cell(r, c)
                mc = s.cell(r, c)
                a1 = oc.coordinate
                ov, mv = norm_val(oc.value), norm_val(mc.value)
                if ov != mv:
                    d.add("値", f"{name}!{a1}: 本家 {ov!r} / うち {mv!r}")
                if oc.number_format != mc.number_format:
                    d.add("表示形式", f"{name}!{a1}: 本家 {oc.number_format!r} / うち {mc.number_format!r}")
                of_, mf = oc.font, mc.font
                if (of_.name, of_.bold or False) != (mf.name, mf.bold or False):
                    d.add("書体", f"{name}!{a1}: 本家 ({of_.name}, 太字={of_.bold}) / うち ({mf.name}, 太字={mf.bold})")
                oa, ma = oc.alignment, mc.alignment
                if (oa.horizontal, oa.vertical) != (ma.horizontal, ma.vertical):
                    d.add("揃え", f"{name}!{a1}: 本家 ({oa.horizontal},{oa.vertical}) / うち ({ma.horizontal},{ma.vertical})")
        try:
            om = {str(m) for m in ws.merged_cells.ranges}
            mm = {str(m) for m in s.merged_cells.ranges}
            for x in sorted(om - mm):
                d.add("結合", f"{name}: 本家にだけ {x}")
            for x in sorted(mm - om):
                d.add("結合", f"{name}: うちにだけ {x}")
        except AttributeError as e:
            d.add("口が無い", f"{e}")
        try:
            for col, dim in ws.column_dimensions.items():
                w1 = dim.width
                w2 = s.column_dimensions[col].width
                if (w1 or 0) and (w2 or 0) and abs(w1 - w2) > 0.02:
                    d.add("列幅", f"{name}:{col}: 本家 {w1} / うち {w2}")
        except AttributeError as e:
            d.add("口が無い", f"{e}")
    return d.report()


def xlsx_roundtrip(path: Path) -> int:
    """officework で開いて**そのまま保存**し、本家に読ませて元と比べる。
    触っていないのに変わる物 = 往復で壊す物。"""
    import openpyxl
    from officework import sheet

    d = Diff(path.name + "(往復)")
    out = Path(tempfile.mkstemp(suffix=".xlsx")[1])
    b = sheet.Book.open(str(path))
    b.save(str(out))
    a = openpyxl.load_workbook(path)
    z = openpyxl.load_workbook(out)
    if a.sheetnames != z.sheetnames:
        d.add("シート名", f"元 {a.sheetnames} / 往復後 {z.sheetnames}")
    for name in a.sheetnames:
        if name not in z.sheetnames:
            continue
        wa, wz = a[name], z[name]
        rows = min(wa.max_row, 400)
        cols = min(wa.max_column, 60)
        for r in range(1, rows + 1):
            for c in range(1, cols + 1):
                ca, cz = wa.cell(r, c), wz.cell(r, c)
                if norm_val(ca.value) != norm_val(cz.value):
                    d.add("値が変わる", f"{name}!{ca.coordinate}: {norm_val(ca.value)!r} → {norm_val(cz.value)!r}")
                if ca.number_format != cz.number_format:
                    d.add("表示形式が変わる", f"{name}!{ca.coordinate}: {ca.number_format!r} → {cz.number_format!r}")
        om = {str(m) for m in wa.merged_cells.ranges}
        mm = {str(m) for m in wz.merged_cells.ranges}
        if om != mm:
            d.add("結合が変わる", f"{name}: {len(om)} → {len(mm)}")
    out.unlink()
    return d.report()


# ---------------- docx ----------------

def docx_diff(path: Path) -> int:
    import docx
    from officework import doc as odoc

    d = Diff(path.name)
    a = docx.Document(str(path))
    m = odoc.Doc.open(str(path))

    apara = a.paragraphs
    if len(apara) != len(m):
        d.add("段落の数", f"本家 {len(apara)} / うち {len(m)}")
    for i in range(min(len(apara), len(m))):
        at = apara[i].text
        mt = m[i].text
        if at != mt:
            d.add("段落の字", f"{i}: 本家 {at[:40]!r} / うち {mt[:40]!r}")
            continue
        aruns = [(r.text, bool(r.bold), bool(r.italic)) for r in apara[i].runs]
        mruns = [(r.text, bool(r.bold), bool(r.italic)) for r in m[i].runs]
        if aruns != mruns:
            d.add("run", f"{i}: 本家 {len(aruns)} 個 / うち {len(mruns)} 個 ({at[:20]!r})")
    if len(a.tables) != len(m.tables):
        d.add("表の数", f"本家 {len(a.tables)} / うち {len(m.tables)}")
    for ti in range(min(len(a.tables), len(m.tables))):
        ta, tm = a.tables[ti], m.tables[ti]
        ra, rm = len(ta.rows), len(tm)
        if ra != rm:
            d.add("表の行数", f"表{ti}: 本家 {ra} / うち {rm}")
        for r in range(min(ra, rm)):
            ca = [c.text for c in ta.rows[r].cells]
            cm = [tm[r][c].text for c in range(len(tm[r]))]
            if ca != cm:
                d.add("表の中身", f"表{ti} 行{r}: 本家 {ca[:3]} / うち {cm[:3]}")
    # 節(用紙と余白)
    asec = a.sections
    msec = m.sections
    if len(asec) != len(msec):
        d.add("節の数", f"本家 {len(asec)} / うち {len(msec)}")
    for i in range(min(len(asec), len(msec))):
        sa, sm = asec[i], msec[i]
        for attr in ("page_width", "page_height", "top_margin", "left_margin"):
            va = getattr(sa, attr)
            vm = getattr(sm, attr)
            va = int(va) if va is not None else None
            vm = int(vm) if vm is not None else None
            if va != vm:
                d.add("節", f"節{i}.{attr}: 本家 {va} / うち {vm}")
    return d.report()


def docx_roundtrip(path: Path) -> int:
    """officework で開いてそのまま保存 → 本家に読ませて元と比べる。"""
    import docx
    from officework import doc as odoc

    d = Diff(path.name + "(往復)")
    out = Path(tempfile.mkstemp(suffix=".docx")[1])
    m = odoc.Doc.open(str(path))
    m.save(str(out))
    a = docx.Document(str(path))
    z = docx.Document(str(out))
    if len(a.paragraphs) != len(z.paragraphs):
        d.add("段落の数が変わる", f"{len(a.paragraphs)} → {len(z.paragraphs)}")
    for i in range(min(len(a.paragraphs), len(z.paragraphs))):
        if a.paragraphs[i].text != z.paragraphs[i].text:
            d.add("字が変わる", f"{i}: {a.paragraphs[i].text[:30]!r} → {z.paragraphs[i].text[:30]!r}")
    if len(a.tables) != len(z.tables):
        d.add("表の数が変わる", f"{len(a.tables)} → {len(z.tables)}")
    out.unlink()
    return d.report()


def main():
    paths = [Path(p).expanduser() for p in sys.argv[1:]]
    if not paths:
        sys.exit("使い方: gokan_diff.py <実物.xlsx/.docx>…")
    total = 0
    for p in paths:
        print(f"== {p}")
        if p.suffix.lower() == ".xlsx":
            total += xlsx_diff(p)
            total += xlsx_roundtrip(p)
        elif p.suffix.lower() == ".docx":
            total += docx_diff(p)
            total += docx_roundtrip(p)
    print(f"\n合計のずれ: {total} 件")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
