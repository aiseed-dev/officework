#!/usr/bin/env python3
"""Compare every template that has a Word PDF beside it, and rank the results.

The folder holds `<guid>.docx` and `<guid>.ms.pdf` side by side (the PDF the
real Word made, see `tools/ms_pdf.py`). Each docx is printed with officework's
own converter and compared with `tools/ms_compare.py`.

    .venv/bin/python tools/templates_diff.py ~/Documents/officework-cmp/templates/en-us out.tsv

The TSV holds one row per template, sorted with the largest difference first:
kind, the first 8 letters of the GUID, Word's page count, ours, and how many
lines `ms_compare` calls different. The count is a place to start, not a mark:
one line that moves can shift every line under it.
"""
import argparse
import glob
import os
import subprocess

import pdfplumber

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCX_PDF = os.path.join(REPO, "target", "release", "examples", "docx_pdf")


def pages(path):
    try:
        with pdfplumber.open(path) as pdf:
            return len(pdf.pages)
    except Exception:  # noqa: BLE001 - a broken PDF is one row, not the end
        return -1


def diffs(ours, ms):
    r = subprocess.run(
        [os.path.join(REPO, ".venv", "bin", "python"), os.path.join(REPO, "tools", "ms_compare.py"),
         ours, ms, "--tol", "3"],
        capture_output=True, text=True, cwd=REPO)
    rows = [l for l in r.stdout.strip().split("\n") if l.strip()]
    if rows and rows[-1].startswith("違い"):
        return int(rows[-1].split()[1].rstrip("件"))
    return 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("folder")
    ap.add_argument("out_tsv")
    a = ap.parse_args()
    rows = []
    for docx in sorted(glob.glob(os.path.join(a.folder, "*", "*.docx"))
                       + glob.glob(os.path.join(a.folder, "*.docx"))):
        base = docx[:-5]
        ms = base + ".ms.pdf"
        if not os.path.exists(ms):
            continue
        kind = os.path.basename(os.path.dirname(base))
        guid = os.path.basename(base)[:8]
        ours = base + ".ours.pdf"
        subprocess.run([DOCX_PDF, docx, ours], capture_output=True, text=True, cwd=REPO)
        rows.append((kind, guid, pages(ms), pages(ours), diffs(ours, ms)))
        print(f"{kind}/{guid}\t{rows[-1][2]}/{rows[-1][3]}\t{rows[-1][4]}")
    rows.sort(key=lambda r: -r[4])
    with open(a.out_tsv, "w", encoding="utf-8") as f:
        for r in rows:
            f.write("\t".join(str(x) for x in r) + "\n")
    print(f"TOTAL {len(rows)} 枚 -> {a.out_tsv}")


if __name__ == "__main__":
    main()
