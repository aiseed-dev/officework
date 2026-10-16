#!/usr/bin/env python3
"""Print the collected Japanese Word and Excel templates with Microsoft 365 and
with officework, and evaluate how officework's print differs (Linux, with the
Windows VM).

    .venv/bin/python tools/templates_ja_round.py         # Word and Excel (needs pdfplumber)
    .venv/bin/python tools/templates_ja_round.py --app excel --limit 10
    .venv/bin/python tools/templates_ja_round.py --skip-ms   # reuse the Word/Excel PDFs

Reads `templates/目録-ja.tsv` and `templates/excel/目録-ja.tsv` (written by
`tools/templates_fetch.py --ja-only`). For every template:

1. Word / Excel in the Windows VM save it as `<GUID>.ms.pdf`
   (`tools/winvm_pdf.py`, all the missing ones in one run of the VM),
2. officework prints it as `<GUID>.ow.pdf` (the `officework` module in .venv),
3. `tools/pdf_eval.py` evaluates the difference by what matters to a reader:
   what is not shown, what is shown wrong, what is hard to read
   (`<GUID>.eval.json`). Page counts and positions are not the measure
   (decided 2026-10-16).

The table goes to `templates/結果-ja.tsv`; `tools/eval_summary.py` adds the
findings up over all templates. The templates are Microsoft's: used for
testing only, never shared (decided 2026-09-19).
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path.home() / "Documents/officework-cmp/templates"
REPO = Path(__file__).resolve().parent.parent
PY = REPO / ".venv/bin/python"
APPS = {"word": (ROOT, ".docx"), "excel": (ROOT / "excel", ".xlsx")}
WEIGHTS = ("表示されない", "改ページ", "違う", "読みにくい")

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

    # 2 and 3. officework, then the evaluation
    table = ["アプリ\tGUID\t頁\t題\t表示されない\t改ページ\t違う\t読みにくい\t主な違い\t誤り"]
    for app, guid, page, title, src in rows:
        ms, ow, ev = src.with_suffix(".ms.pdf"), src.with_suffix(".ow.pdf"), src.with_suffix(".eval.json")
        err, res = "", {}
        try:
            r = subprocess.run([str(PY), "-c", OW, str(src), str(ow)], capture_output=True, text=True, timeout=300)
            if r.returncode or not ow.exists():
                err = (r.stderr.strip().splitlines() or ["officework で PDF にできない"])[-1][:200]
            elif not ms.exists():
                err = "Word・Excel の PDF が無い"
            else:
                r = subprocess.run([str(PY), str(REPO / "tools/pdf_eval.py"), str(ow), str(ms), "--source", str(src), "--json", str(ev)],
                                   capture_output=True, text=True, timeout=900)
                if ev.exists():
                    res = json.loads(ev.read_text(encoding="utf-8"))
                else:
                    err = (r.stderr.strip().splitlines() or ["評価できない"])[-1][:200]
        except subprocess.TimeoutExpired:
            err = "時間切れ"
        counts = [sum(len(v) for v in res.get(w, {}).values()) for w in WEIGHTS]
        kinds = sorted(((len(v), f"{k}") for w in WEIGHTS for k, v in res.get(w, {}).items()), reverse=True)
        main_kinds = "、".join(f"{k} {n}" for n, k in kinds[:3])
        table.append("\t".join(map(str, [app, guid, page, title, *counts, main_kinds, err])))
        print(f"{app} {title[:28]}: 表示されない {counts[0]} / 改ページ {counts[1]} / 違う {counts[2]} / 読みにくい {counts[3]}  {main_kinds} {err}",
              flush=True)
    # A run of one app keeps the other app's rows from the last run
    old = ROOT / "結果-ja.tsv"
    if old.exists():
        lines = old.read_text(encoding="utf-8").splitlines()
        if lines and lines[0] == table[0]:
            table += [ln for ln in lines[1:] if ln.split("\t", 1)[0] not in apps]
    old.write_text("\n".join(table) + "\n", encoding="utf-8")
    print(f"\n表: {old}  まとめ: .venv/bin/python tools/eval_summary.py")


if __name__ == "__main__":
    main()
