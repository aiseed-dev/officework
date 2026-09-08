#!/usr/bin/env bash
# **受け入れ試験の10枚を作り、officework と Word / Excel の PDF を比べる1周。**
#
#   bash tools/ms_round.sh [出力先(既定 ~/Documents/officework-cmp/out)]
#
# 1. test/write_docx_officework.py と write_xlsx_officework.py で10枚を書く
# 2. officework で PDF に(<名前>.ow.pdf)
# 3. Word / Excel で PDF に(<名前>.ms.pdf。tools/ms_pdf.py)
# 4. tools/ms_compare.py で比べ、違いを <名前>.diff.txt に置く
#
# 出力先は ~/Documents の下にします。Excel は /private/tmp の下へ書けません。
# 前に Python のエンジンを直したときは `cd pysheet && ../.venv/bin/maturin develop --release`。
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
  python3 "$ROOT/tools/ms_pdf.py" "$f" "$n.ms.pdf" || { echo "× $f: Word/Excel で PDF にできない"; continue; }
  "$PY" "$ROOT/tools/ms_compare.py" "$n.ow.pdf" "$n.ms.pdf" > "$n.diff.txt"
  echo "== $n: $(tail -1 "$n.diff.txt")"
done
