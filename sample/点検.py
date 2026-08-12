# フォルダの xlsx / docx を全部開いて「読めなかった部品」を数える(アプリ不要)。
#
#   pip install officework
#   python3 点検.py [フォルダ]
#
# このエンジンは理解できない部品を**黙って落とさず** unsupported に出す。
# 受け取った帳票の束を差し込みに使う前の門 — ここが空なら安心して回せる。
import sys
import pathlib
from officework import sheet, doc

folder = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
n_ok = 0
n_bad = 0
for p in sorted(folder.iterdir()):
    try:
        if p.suffix == ".xlsx":
            u = sheet.Book.open(str(p)).unsupported
        elif p.suffix == ".docx":
            u = doc.Doc.open(str(p)).unsupported
        else:
            continue
    except Exception as e:
        print(f"×開けません {p.name}: {e}")
        n_bad += 1
        continue
    if u:
        print(f"△ {p.name}: {len(u)} 件 — {u[:3]}")
        n_bad += 1
    else:
        n_ok += 1
print(f"きれい {n_ok} 本 / 気になる {n_bad} 本")
