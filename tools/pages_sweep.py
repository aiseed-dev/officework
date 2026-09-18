#!/usr/bin/env python3
"""Print every docx in a folder and compare the page count with Word's PDF.

A folder that holds `<name>.docx` and `<name>.ms.pdf` side by side (the PDF made
by the real Word, see `tools/ms_pdf.py`) is the input. Every docx that has a
`.ms.pdf` next to it is printed with officework's own converter and the two page
counts are written to a TSV.

    python3 tools/pages_sweep.py ~/Documents/officework-cmp/corpus out.tsv
    python3 tools/pages_sweep.py CORPUS out.tsv --skip a.docx,b.docx
    python3 tools/pages_sweep.py CORPUS out.tsv --skip-file はずす.txt

The converter is `target/release/examples/docx_pdf`
(`cargo build --release -p paper --example docx_pdf`). Our PDF is written to a
temporary folder and removed again. A file our converter fails on is written
with -1 pages and the run keeps going.
"""
import argparse
import glob
import os
import subprocess
import sys
import tempfile

import pdfplumber

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCX_PDF = os.path.join(REPO, "target", "release", "examples", "docx_pdf")


def pages(path):
    """Page count of a PDF, or -1 when it cannot be read."""
    try:
        with pdfplumber.open(path) as pdf:
            return len(pdf.pages)
    except Exception:  # noqa: BLE001 - a broken PDF is one row, not the end
        return -1


def sweep(folder, out_tsv, skip=()):
    skip = set(skip)
    rows, total, match = [], 0, 0
    with tempfile.TemporaryDirectory() as tmp:
        for docx in sorted(glob.glob(os.path.join(folder, "*.docx"))):
            name = os.path.basename(docx)
            if name in skip or name[:-5] in skip:
                continue
            ms = docx[:-5] + ".ms.pdf"
            if not os.path.exists(ms):
                continue
            ours_pdf = os.path.join(tmp, name[:-5] + ".pdf")
            subprocess.run([DOCX_PDF, docx, ours_pdf], capture_output=True, text=True, cwd=REPO)
            ours, word = pages(ours_pdf), pages(ms)
            total += 1
            match += ours == word
            rows.append(f"{name[:-5]}\t{ours}\t{word}")
            try:
                os.remove(ours_pdf)
            except OSError:
                pass
    text = "名前\tうちの頁数\tWord の頁数\n" + "\n".join(rows) + f"\nTOTAL {total} match {match}\n"
    with open(out_tsv, "w", encoding="utf-8") as f:
        f.write(text)
    return total, match


def main(argv=None):
    p = argparse.ArgumentParser(description="docx を刷って Word の PDF と頁数を比べます")
    p.add_argument("folder", help="docx と .ms.pdf の置いてあるフォルダ")
    p.add_argument("out", help="書き出す TSV")
    p.add_argument("--skip", default="", help="はずすファイル名をコンマで並べます")
    p.add_argument("--skip-file", default="", help="はずすファイル名を 1 行に 1 つ書いたファイル")
    a = p.parse_args(argv)

    skip = [s.strip() for s in a.skip.split(",") if s.strip()]
    if a.skip_file:
        skip += [s.strip() for s in open(a.skip_file, encoding="utf-8").read().splitlines() if s.strip()]
    if not os.path.exists(DOCX_PDF):
        raise SystemExit(f"変換器がありません: {DOCX_PDF}\n"
                         "cargo build --release -p paper --example docx_pdf")
    total, match = sweep(os.path.expanduser(a.folder), a.out, skip)
    print(f"TOTAL {total} match {match} -> {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
