#!/usr/bin/env python3
"""Print every ods of a folder with LibreOffice and with officework, and
compare the two PDFs.

    .venv/bin/python tools/corpus_round_ods.py [--corpus DIR] [--out DIR]
                                              [--only NAME] [--redo-lo]

The ods files (default `~/ods-corpus/`) go to `tools/lo_pdf.py` for the
reference PDFs and to `cargo run -p ops --example ods_pdf` for ours. Each pair
is judged by `tools/pdf_eval.py`. The folder `--out` (default
`~/Documents/officework-cmp/corpus-ods/`) gets `<name>.lo.pdf`, `<name>.ow.pdf`
and `<name>.eval.json`, and the summary below is printed: what is found, how
many documents it touches, by weight. What touches the most documents comes
first.

LibreOffice's PDFs are made once and kept (`--redo-lo` makes them again);
ours are made every time, since the point is to run this after a fix. The
PDFs depend on the LibreOffice version, which `lo_pdf.py` prints at its start.
`docs/sekkei/odf-bunseki.ja.adoc` has what the first rounds found.

Kinds of difference that are a matter of faces and not of the document (the
weight and the kind of a Japanese face) are left out of the summary, since
LibreOffice picks a face by fontconfig on this machine; `--all` lists them.
"""
import argparse
import glob
import json
import os
import subprocess
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PY = os.path.join(ROOT, ".venv", "bin", "python")
WEIGHTS = ("表示されない", "改ページ", "違う", "読みにくい")
FACE_KINDS = ("書体の種類", "字の太さ")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default=os.path.expanduser("~/ods-corpus"))
    ap.add_argument("--out", default=os.path.expanduser("~/Documents/officework-cmp/corpus-ods"))
    ap.add_argument("--only", default="")
    ap.add_argument("--redo-lo", action="store_true")
    ap.add_argument("--all", action="store_true")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    files = sorted(glob.glob(os.path.join(a.corpus, "*.ods")))
    if a.only:
        files = [f for f in files if a.only in os.path.basename(f)]
    if not files:
        print("no ods files", file=sys.stderr)
        return 2

    # The reference PDFs, only for what has none
    todo = [
        f for f in files
        if a.redo_lo or not os.path.exists(os.path.join(a.out, os.path.basename(f)[:-4] + ".lo.pdf"))
    ]
    if todo:
        subprocess.run([sys.executable, os.path.join(ROOT, "tools/lo_pdf.py"), *todo, "--out", a.out], check=False)

    # One build, then a run for each file
    env = dict(os.environ, PYO3_PYTHON=PY)
    b = subprocess.run(["cargo", "build", "-q", "-p", "ops", "--example", "ods_pdf"], cwd=ROOT, env=env)
    if b.returncode:
        return b.returncode
    exe = os.path.join(ROOT, "target/debug/examples/ods_pdf")

    found: dict = defaultdict(lambda: defaultdict(lambda: [0, 0, None]))
    n = 0
    for f in files:
        name = os.path.basename(f)[:-4]
        lo, ow, ej = (os.path.join(a.out, name + s) for s in (".lo.pdf", ".ow.pdf", ".eval.json"))
        r = subprocess.run([exe, f, ow], capture_output=True)
        if r.returncode or not os.path.exists(lo):
            print(f"skipped {name}: no PDF of ours or of LibreOffice")
            continue
        subprocess.run([PY, os.path.join(ROOT, "tools/pdf_eval.py"), ow, lo, "--source", f, "--json", ej],
                       capture_output=True)
        try:
            d = json.load(open(ej, encoding="utf-8"))
        except (OSError, ValueError):
            continue
        n += 1
        for w in WEIGHTS:
            for kind, items in d.get(w, {}).items():
                if kind in FACE_KINDS and not a.all:
                    continue
                e = found[w][kind]
                e[0] += 1
                e[1] += len(items)
                if e[2] is None:
                    e[2] = (name, items[0])
    print(f"\n{n} documents compared\n")
    for w in WEIGHTS:
        if not found[w]:
            continue
        print(f"■ {w}")
        for kind, (docs, count, ex) in sorted(found[w].items(), key=lambda kv: -kv[1][0]):
            print(f"  {kind}: {docs} documents, {count} findings; e.g. {ex[0]}: {str(ex[1])[:80]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
