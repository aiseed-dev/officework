#!/usr/bin/env python3
"""**集めた xlsx を、officework と Excel の両方で PDF にして比べる1周。**

`tools/corpus_fetch.py --ext xlsx` で落とした xlsx(既定 `~/xlsx-corpus/`)を
1枚ずつ、officework(`Book.open().to_pdf()`)と Excel(`tools/ms_pdf.py`)で
PDF にし、`tools/ms_compare.py` で比べます。結果は出力先(既定
`~/Documents/officework-cmp/corpus-xlsx/`)に `<名前>.ow.pdf` / `.ms.pdf` /
`.diff.txt` と、まとめの `まとめ.tsv`(違いの多い順)を置きます。
docx 用の `tools/corpus_round.py` と同じ形です。

    .venv/bin/python tools/corpus_round_xlsx.py [--corpus DIR] [--out DIR]
                                               [--limit N] [--only 名前]
                                               [--ms-only] [--redo-ms]

Excel の PDF は1度作れば取っておきます(`--redo-ms` で作り直し)。
officework の PDF は毎回作り直します(直した後に回すのが目的なので)。

## この道具の決め

* **Excel に開かせるのは出力先に置いた写しです。** `~/Documents` の外の
  ファイルだと Excel が「アクセスを許可するか」を画面で聞いて、AppleScript が
  返らなくなります
* **`.xlsm` は飛ばします。** マクロつきのブックは Excel が有効化を聞くので、
  画面の見えない所では止まります
* Excel の PDF は `tools/ms_pdf.py` の `excel_pdf_many` でまとめて作ります。
  Excel の起動と終了は1回だけです
* 発注者の Word には触りません(Excel だけを落とします)
"""
import argparse
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tools"))
import ms_compare  # noqa: E402
import ms_pdf  # noqa: E402

PY = os.path.join(ROOT, ".venv", "bin", "python")
ENV = dict(os.environ, DYLD_FALLBACK_LIBRARY_PATH=os.path.expanduser("~/miniforge3/lib"))


def ours(src, out):
    """officework で PDF にします。誤りがあればその文言、無ければ None。"""
    code = ("import sys\nfrom officework import sheet\n"
            "sheet.Book.open(sys.argv[1]).to_pdf(sys.argv[2])\n")
    try:
        r = subprocess.run([PY, "-c", code, src, out], capture_output=True, text=True,
                           timeout=300, env=ENV)
    except subprocess.TimeoutExpired:
        return "うちが 5 分返さない"
    if r.returncode != 0:
        return r.stderr.strip().splitlines()[-1][:200] if r.stderr.strip() else f"exit {r.returncode}"
    return None


def copy_in(src, dst):
    import shutil
    if not os.path.exists(dst) or os.path.getmtime(dst) < os.path.getmtime(src):
        shutil.copyfile(src, dst)
    return dst


def pages(pdf):
    import pdfplumber
    with pdfplumber.open(pdf) as p:
        return len(p.pages)


def collect(corpus, only, limit):
    files = []
    for d, _, fs in os.walk(corpus):
        for f in sorted(fs):
            # `.xlsm` はマクロの有効化を Excel が聞くので回しません
            if f.lower().endswith(".xlsx") and not f.startswith("~$"):
                files.append(os.path.join(d, f))
    if only:
        files = [f for f in files if only in f]
    return files[:limit] if limit else files


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default=os.path.expanduser("~/xlsx-corpus"))
    ap.add_argument("--out", default=os.path.expanduser("~/Documents/officework-cmp/corpus-xlsx"))
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--only", default=None)
    ap.add_argument("--redo-ms", action="store_true")
    ap.add_argument("--ms-only", action="store_true", help="Excel の PDF だけ作る(うちの側は後で)")
    ap.add_argument("--tol", type=float, default=6.0)
    a = ap.parse_args()
    files = collect(a.corpus, a.only, a.limit)
    os.makedirs(a.out, exist_ok=True)

    # 出力先に写しを置き、Excel にはそれを開かせます
    plan = []
    for src in files:
        rel = os.path.relpath(src, a.corpus)
        stem = os.path.join(a.out, rel[:-5].replace(os.sep, "__"))
        plan.append((rel, copy_in(src, stem + ".xlsx"), stem))

    # Excel の PDF は、無い物だけまとめて作ります
    todo = [(c, s + ".ms.pdf") for _, c, s in plan
            if a.redo_ms or not os.path.exists(s + ".ms.pdf")]
    ms_err = {}
    if todo:
        print(f"== Excel に {len(todo)} 枚を PDF にさせます", flush=True)
        t0 = time.time()

        def done(src, err):
            ms_err[src] = err
            print(f"{'×' if err else '+'} {os.path.basename(src)}"
                  f"{': ' + err if err else ''} {time.time() - t0:.0f}s", flush=True)

        ms_pdf.excel_pdf_many(todo, on_done=done)
    if a.ms_only:
        return

    rows = []
    for rel, src, stem in plan:
        ow, ms, diff = stem + ".ow.pdf", stem + ".ms.pdf", stem + ".diff.txt"
        t0 = time.time()
        e = ours(src, ow)
        if e:
            rows.append((rel, -1, 0, 0, f"うちが PDF にできない: {e}"))
            print(f"× {rel}: うち: {e}", flush=True)
            continue
        if not os.path.exists(ms):
            rows.append((rel, -1, pages(ow), 0,
                         f"Excel が PDF にできない: {ms_err.get(src) or '出ていない'}"))
            print(f"× {rel}: Excel: {ms_err.get(src)}", flush=True)
            continue
        d = ms_compare.compare(ow, ms, tol=a.tol)
        with open(diff, "w", encoding="utf-8") as f:
            f.write("\n".join(d) + f"\n違い {len(d)} 件\n")
        po, pm = pages(ow), pages(ms)
        rows.append((rel, len(d), po, pm, d[0] if d else ""))
        print(f"== {rel}: 違い {len(d)} 件 (頁 {po}/{pm}) {time.time() - t0:.0f}s", flush=True)

    rows.sort(key=lambda r: (-r[1], r[0]))
    with open(os.path.join(a.out, "まとめ.tsv"), "w", encoding="utf-8") as f:
        f.write("ファイル\t違い\tうちの頁\tExcel の頁\t最初の違い\n")
        for r in rows:
            f.write("\t".join(str(x) for x in r) + "\n")
    ok = sum(1 for r in rows if r[1] == 0)
    print(f"一致 {ok} / {len(rows)}")


if __name__ == "__main__":
    main()
