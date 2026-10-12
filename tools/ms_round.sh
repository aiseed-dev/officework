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
#
# The output goes under ~/Documents: Excel on the Mac cannot write under
# /private/tmp. After changing the engine, reinstall it first:
# `cd pysheet && ../.venv/bin/maturin develop --release`.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$HOME/Documents/officework-cmp/out}"
PY="$ROOT/.venv/bin/python"
mkdir -p "$OUT"
rm -f "$OUT"/*.pdf "$OUT"/*.diff.txt
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
  if [ "$(uname)" = Linux ]; then
    python3 "$ROOT/tools/winvm_pdf.py" "$f" >/dev/null || { echo "× $f: Word/Excel で PDF にできない"; continue; }
  else
    python3 "$ROOT/tools/ms_pdf.py" "$f" "$n.ms.pdf" || { echo "× $f: Word/Excel で PDF にできない"; continue; }
  fi
  "$PY" "$ROOT/tools/ms_compare.py" "$n.ow.pdf" "$n.ms.pdf" > "$n.diff.txt"
  echo "== $n: $(tail -1 "$n.diff.txt")"
done
