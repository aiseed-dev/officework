#!/usr/bin/env python3
"""Print the collected Japanese Word and Excel templates with Microsoft 365 and
with officework, and compare them (Linux, with the Windows VM).

    .venv/bin/python tools/templates_ja_round.py         # Word and Excel (needs pdfplumber)
    .venv/bin/python tools/templates_ja_round.py --app excel --limit 10
    .venv/bin/python tools/templates_ja_round.py --skip-ms   # reuse the Word/Excel PDFs

Reads `templates/目録-ja.tsv` and `templates/excel/目録-ja.tsv` (written by
`tools/templates_fetch.py --ja-only`). For every template:

1. Word / Excel in the Windows VM save it as `<GUID>.ms.pdf`
   (`tools/winvm_pdf.py`, all the missing ones in one run of the VM),
2. officework prints it as `<GUID>.ow.pdf` (the `officework` module in .venv),
3. the page counts are compared, and page by page the letters
   (`tools/ms_compare.py`: lines beyond 6pt and beyond 2pt, and letters of
   another colour, letters out of place within a line), the ruled lines
   (`tools/ms_rules.py`) and the pictures (`tools/ms_imgdiff.py`, differing
   8-pixel squares, and those outside the letters).

A template "matches" when both have the same number of pages, no line is more
than 6pt away and no letter has another colour. The table goes to `templates/結果-ja.tsv`, with the error
text of a file that failed. The templates are Microsoft's: used for testing
only, never shared (decided 2026-09-19).
"""
import argparse
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path.home() / "Documents/officework-cmp/templates"
REPO = Path(__file__).resolve().parent.parent
PY = REPO / ".venv/bin/python"
APPS = {"word": (ROOT, ".docx"), "excel": (ROOT / "excel", ".xlsx")}

OW = """
import sys
from officework import doc, sheet
src, out = sys.argv[1], sys.argv[2]
if src.endswith(".docx"):
    doc.Doc.open(src).to_pdf(out)
else:
    sheet.Book.open(src).to_pdf(out)
"""


def rows_of(app):
    root, ext = APPS[app]
    tsv = root / "目録-ja.tsv"
    if not tsv.exists():
        return []
    out = []
    for line in tsv.read_text(encoding="utf-8").splitlines()[1:]:
        f = line.split("\t")
        if len(f) >= 3:
            out.append((app, f[0], f[1], f[2], root / "ja-jp" / (f[0] + ext)))
    return out


def pages(pdf):
    import pdfplumber
    with pdfplumber.open(pdf) as p:
        return len(p.pages)


def last_number(text):
    nums = [int(t) for t in text.replace("件", " ").replace("区画", " ").split() if t.isdigit()]
    return nums[-1] if nums else -1


def compare(ow, ms):
    out = {}
    for tol in ("6", "2"):
        r = subprocess.run([str(PY), str(REPO / "tools/ms_compare.py"), str(ow), str(ms), "--tol", tol],
                           capture_output=True, text=True, timeout=300)
        lines = r.stdout.strip().splitlines() or ["-1"]
        out["lines" + tol] = last_number(lines[-1])
        if tol == "6":
            out["colour"] = next((last_number(x) for x in lines if x.startswith("色の違い")), -1)
    r = subprocess.run([str(PY), str(REPO / "tools/ms_imgdiff.py"), str(ow), str(ms),
                        "--out", str(ow.with_suffix(".imgdiff.png"))],
                       capture_output=True, text=True, timeout=600)
    lines = r.stdout.strip().splitlines() or ["-1"]
    out["img"] = last_number(lines[-1])
    out["img_out"] = next((last_number(x) for x in lines if x.startswith("字の外の違い")), -1)
    r = subprocess.run([str(PY), str(REPO / "tools/ms_rules.py"), str(ow), str(ms)],
                       capture_output=True, text=True, timeout=300)
    out["rules"] = last_number((r.stdout.strip().splitlines() or ["-1"])[-1])
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--app", choices=["word", "excel", "both"], default="both")
    ap.add_argument("--limit", type=int, default=0, help="templates per app (0: all)")
    ap.add_argument("--skip-ms", action="store_true", help="never start the VM; reuse the PDFs there are")
    a = ap.parse_args()
    apps = ["word", "excel"] if a.app == "both" else [a.app]
    rows = []
    for app in apps:
        rs = [r for r in rows_of(app) if r[4].exists()]
        rows += rs[:a.limit] if a.limit else rs
    if not rows:
        raise SystemExit("no templates; run tools/templates_fetch.py --ja-only first")

    # 1. Word / Excel, in one run of the VM
    missing = [r[4] for r in rows if not r[4].with_suffix(".ms.pdf").exists()]
    if missing and not a.skip_ms:
        print(f"Word / Excel: {len(missing)} files", flush=True)
        subprocess.run([sys.executable, str(REPO / "tools/winvm_pdf.py"), *map(str, missing),
                        "--timeout", str(120 + 60 * len(missing))])

    # 2 and 3. officework, then the comparison
    table = ["アプリ\tGUID\t頁\t題\tWord・Excel の頁数\tofficework の頁数\t6pt を超えた行\t2pt を超えた行\t字の色の違い\t罫線の違い\t絵の違う区画\t字の外の違う区画\t一致\t誤り"]
    n = n_ok = 0
    for app, guid, page, title, src in rows:
        ms, ow = src.with_suffix(".ms.pdf"), src.with_suffix(".ow.pdf")
        err, res, pm, po = "", {}, -1, -1
        try:
            r = subprocess.run([str(PY), "-c", OW, str(src), str(ow)], capture_output=True, text=True, timeout=300)
            if r.returncode or not ow.exists():
                err = (r.stderr.strip().splitlines() or ["officework で PDF にできない"])[-1][:200]
            elif not ms.exists():
                err = "Word・Excel の PDF が無い"
            else:
                pm, po = pages(ms), pages(ow)
                res = compare(ow, ms)
        except subprocess.TimeoutExpired:
            err = "時間切れ"
        # lines6 counts the colour differences too
        ok = not err and pm == po and res.get("lines6") == 0
        n += 1
        n_ok += ok
        table.append("\t".join(map(str, [app, guid, page, title, pm, po, res.get("lines6", ""),
                                          res.get("lines2", ""), res.get("colour", ""), res.get("rules", ""), res.get("img", ""), res.get("img_out", ""), "○" if ok else "", err])))
        print(f"{'○' if ok else '×'} {app} {title[:30]} 頁 {pm}/{po} 行6pt {res.get('lines6', '-')} "
              f"色 {res.get('colour', '-')} 罫線 {res.get('rules', '-')} 絵 {res.get('img', '-')}({res.get('img_out', '-')}) {err}", flush=True)
    (ROOT / "結果-ja.tsv").write_text("\n".join(table) + "\n", encoding="utf-8")
    print(f"\n一致 {n_ok} / {n}  (表: {ROOT / '結果-ja.tsv'})")


if __name__ == "__main__":
    main()
