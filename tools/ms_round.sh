#!/usr/bin/env bash
# One round of the acceptance test: write the ten files and compare
# officework's PDF of each with Word's or Excel's.
#
#   bash tools/ms_round.sh [output folder (default ~/Documents/officework-cmp/out)]
#
# 1. Write the ten files with test/write_docx_officework.py and
#    write_xlsx_officework.py
# 2. officework saves each as PDF (<name>.ow.pdf)
# 3. Word / Excel save each as PDF (<name>.ms.pdf): tools/ms_pdf.py on a Mac,
#    tools/winvm_pdf.py (Word / Excel in the Windows VM) on Linux
# 4. tools/ms_compare.py compares the two and writes <name>.diff.txt
# 5. tools/ms_imgdiff.py compares them as pictures (fills, lines, what
#    covers what) and writes <name>.imgdiff.txt and <name>.imgdiff.png
#
# The output goes under ~/Documents: Excel on the Mac cannot write under
# /private/tmp. After changing the engine, reinstall it first:
# `cd pysheet && ../.venv/bin/maturin develop --release`.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$HOME/Documents/officework-cmp/out}"
PY="$ROOT/.venv/bin/python"
mkdir -p "$OUT"
rm -f "$OUT"/*.pdf "$OUT"/*.diff.txt "$OUT"/*.imgdiff.txt "$OUT"/*.imgdiff.png
"$PY" "$ROOT/test/write_docx_officework.py" "$OUT" || exit 1
"$PY" "$ROOT/test/write_xlsx_officework.py" "$OUT" || exit 1
cd "$OUT" || exit 1
for f in *.docx *.xlsx; do
  n="${f%.*}"
  "$PY" - "$f" "$n.ow.pdf" <<'EOF'
import sys
from officework import doc, sheet
src, out = sys.argv[1], sys.argv[2]
if src.endswith(".docx"):
    doc.Doc.open(src).to_pdf(out)
else:
    sheet.Book.open(src).to_pdf(out)
EOF
done
# On Linux all the files go to the Windows VM in one run: it starts the VM if
# needed and shuts it down afterwards, so once for the whole round
if [ "$(uname)" = Linux ]; then
  python3 "$ROOT/tools/winvm_pdf.py" *.docx *.xlsx >/dev/null || echo "× some files: Word/Excel could not save them as PDF"
fi
for f in *.docx *.xlsx; do
  n="${f%.*}"
  if [ "$(uname)" != Linux ]; then
    python3 "$ROOT/tools/ms_pdf.py" "$f" "$n.ms.pdf" || { echo "× $f: Word/Excel で PDF にできない"; continue; }
  fi
  [ -f "$n.ms.pdf" ] || { echo "× $f: no PDF from Word/Excel"; continue; }
  "$PY" "$ROOT/tools/ms_compare.py" "$n.ow.pdf" "$n.ms.pdf" > "$n.diff.txt"
  "$PY" "$ROOT/tools/ms_imgdiff.py" "$n.ow.pdf" "$n.ms.pdf" --out "$n.imgdiff.png" > "$n.imgdiff.txt"
  echo "== $n: $(tail -1 "$n.diff.txt") / $(tail -1 "$n.imgdiff.txt")"
done
