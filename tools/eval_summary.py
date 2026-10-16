#!/usr/bin/env python3
"""Add up the findings of `tools/pdf_eval.py` over many documents.

    .venv/bin/python tools/eval_summary.py [folder …]

Reads every `*.eval.json` under the folders (by default the template
folders) and lists the kinds of difference by weight (not shown, shown wrong,
hard to read), each with the number of documents it touches, the number of
findings, and examples with the document they come from. What touches the
most documents comes first: that is what to fix first.
"""
import json
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path.home() / "Documents/officework-cmp/templates"
WEIGHTS = ("表示されない", "改ページ", "違う", "読みにくい")


def titles():
    out = {}
    for tsv in (ROOT / "目録-ja.tsv", ROOT / "excel/目録-ja.tsv"):
        if tsv.exists():
            for line in tsv.read_text(encoding="utf-8").splitlines()[1:]:
                f = line.split("\t")
                if len(f) >= 3:
                    out[f[0]] = f[2]
    return out


def main():
    folders = [Path(p) for p in sys.argv[1:]] or [ROOT / "ja-jp", ROOT / "excel/ja-jp"]
    names = titles()
    docs = defaultdict(set)
    count = defaultdict(int)
    examples = defaultdict(list)
    n = 0
    for folder in folders:
        for ev in sorted(folder.glob("*.eval.json")):
            n += 1
            guid = ev.name.split(".")[0]
            app = "Excel" if "excel" in str(folder) else "Word"
            res = json.loads(ev.read_text(encoding="utf-8"))
            for w in WEIGHTS:
                for kind, items in res.get(w, {}).items():
                    key = (w, kind)
                    docs[key].add(guid)
                    count[key] += len(items)
                    if len(examples[key]) < 3:
                        examples[key].append(f"{app}「{names.get(guid, guid)[:20]}」: {items[0][:70]}")
    print(f"評価した文書 {n}")
    for w in WEIGHTS:
        keys = sorted((k for k in docs if k[0] == w), key=lambda k: (-len(docs[k]), -count[k]))
        print(f"\n■ {w}")
        for k in keys:
            print(f"  {k[1]}: {len(docs[k])} 文書、{count[k]} 件")
            for e in examples[k]:
                print(f"      {e}")


if __name__ == "__main__":
    main()
