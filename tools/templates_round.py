#!/usr/bin/env python3
"""Print every collected Word template with Word and with officework, and compare.

Reads `~/Documents/officework-cmp/templates/目録.tsv` (written by
`tools/templates_fetch.py`) and, for both locales of every row:

1. has the real Word save the docx as `<GUID>.ms.pdf` when that file is missing
   (`tools/ms_pdf.word_pdf`; one document at a time, closed again afterwards),
2. prints the same docx with `target/release/examples/docx_pdf` to
   `<GUID>.ours.pdf`,
3. compares the page counts and the lines of page 1.

The line score is the share of Word's page-1 text lines for which our page 1 has
a line within 2pt of the same top position whose text starts with the same
characters. It is a rough measure of whether the text sits in the same place, not
a pixel comparison.

    python3 tools/templates_round.py
    python3 tools/templates_round.py --limit 10
    python3 tools/templates_round.py --only ce343500-4aff-4dfa-b337-57c78459c6ee
    python3 tools/templates_round.py --skip-word

`--skip-word` reuses the PDFs that are already there and never starts Word. The
result is written to `templates/結果.tsv`. A file that fails is written with its
error text and the run keeps going.
"""
import argparse
import os
import subprocess
import sys

import pdfplumber

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ms_pdf  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCX_PDF = os.path.join(REPO, "target", "release", "examples", "docx_pdf")
ROOT = os.path.expanduser("~/Documents/officework-cmp/templates")
LOCALE = {"en": "en-us", "ja": "ja-jp"}
TOL = 2.0  # how far apart two lines may sit and still count as the same line
HEAD = 3   # how many characters at the start of a line have to agree


class Tomeru(Exception):
    """Word will not close its documents; the run has to stop here."""


def lines(page):
    """Group the words of a page into lines of (top position, text)."""
    words = sorted(page.extract_words(use_text_flow=False), key=lambda w: (w["top"], w["x0"]))
    out = []
    for w in words:
        if out and abs(w["top"] - out[-1][0]) <= 1.5:
            out[-1][1].append(w)
        else:
            out.append([w["top"], [w]])
    rows = []
    for top, ws in out:
        text = "".join(x["text"] for x in sorted(ws, key=lambda x: x["x0"]))
        if text.strip():
            rows.append((top, text.strip()))
    return rows


def page_lines(path):
    with pdfplumber.open(path) as pdf:
        if not pdf.pages:
            return 0, []
        return len(pdf.pages), lines(pdf.pages[0])


def score(word_rows, our_rows):
    """Share of Word's lines that we put in the same place with the same start."""
    if not word_rows:
        return 1.0 if not our_rows else 0.0
    hit = 0
    for top, text in word_rows:
        head = text[:HEAD]
        for t2, text2 in our_rows:
            if abs(t2 - top) <= TOL and text2[:HEAD] == head:
                hit += 1
                break
    return hit / len(word_rows)


def ours_pdf(docx, out):
    r = subprocess.run([DOCX_PDF, docx, out], capture_output=True, text=True, cwd=REPO)
    if not os.path.exists(out):
        msg = (r.stderr or r.stdout).strip().splitlines()
        raise RuntimeError(msg[-1][:200] if msg else f"docx_pdf が終了コード {r.returncode}")


def read_mokuroku(path):
    rows = []
    for line in open(path, encoding="utf-8").read().splitlines()[1:]:
        c = line.split("\t")
        if len(c) >= 4 and c[0].strip():
            rows.append(c)
    return rows


def one(guid, kind, title, lang, skip_word):
    """One locale of one template. Returns the row for 結果.tsv."""
    base = os.path.join(ROOT, LOCALE[lang], guid)
    docx, ms, our = base + ".docx", base + ".ms.pdf", base + ".ours.pdf"
    row = [guid, LOCALE[lang], kind, title, "", "", "", "", ""]
    if not os.path.exists(docx):
        row[8] = "docx がありません"
        return row
    if not os.path.exists(ms) and skip_word:
        # Without Word's PDF there is nothing to compare, but printing it with our
        # own converter still shows whether we can read the docx at all.
        row[8] = "Word の PDF がありません(--skip-word)"
        try:
            ours_pdf(docx, our)
            row[5] = str(page_lines(our)[0])
        except Exception as e:  # noqa: BLE001 - one failure is one row
            row[8] += " / " + str(e).splitlines()[-1][:200]
        return row
    if not os.path.exists(ms):
        try:
            ms_pdf.word_pdf(docx, ms)
        except Exception as e:  # noqa: BLE001 - one file must not stop the run
            # The one thing that does stop the run: Word will not close what it
            # opened. Going on would leave window after window open.
            if "閉じません" in str(e):
                raise Tomeru(str(e)) from e
            row[8] = "Word: " + (str(e).splitlines()[-1][:200] if str(e).strip() else type(e).__name__)
            return row
        if not os.path.exists(ms):
            row[8] = "Word が PDF を書きませんでした"
            return row
    try:
        ours_pdf(docx, our)
    except Exception as e:  # noqa: BLE001 - one failure is one row
        row[8] = str(e).splitlines()[-1][:200]
    try:
        w_pages, w_rows = page_lines(ms)
        row[4] = str(w_pages)
    except Exception as e:  # noqa: BLE001
        row[8] = (row[8] + " / " if row[8] else "") + f"Word の PDF が読めません: {e}"
        return row
    if not os.path.exists(our):
        return row
    try:
        o_pages, o_rows = page_lines(our)
    except Exception as e:  # noqa: BLE001
        row[8] = (row[8] + " / " if row[8] else "") + f"うちの PDF が読めません: {e}"
        return row
    row[5] = str(o_pages)
    row[6] = "○" if o_pages == w_pages else "×"
    row[7] = f"{score(w_rows, o_rows):.3f}"
    return row


def main(argv=None):
    global ROOT
    p = argparse.ArgumentParser(description="集めたテンプレートを Word とうちで刷って比べます")
    p.add_argument("--limit", type=int, default=0, help="目録の先頭から何組までか(0 は全部)")
    p.add_argument("--only", default="", help="この GUID だけ")
    p.add_argument("--skip-word", action="store_true", help="Word を動かさず、ある PDF を使います")
    p.add_argument("--root", default=ROOT, help="置き場")
    a = p.parse_args(argv)

    ROOT = os.path.expanduser(a.root)
    mokuroku = os.path.join(ROOT, "目録.tsv")
    if not os.path.exists(mokuroku):
        raise SystemExit(f"目録がありません: {mokuroku}(先に tools/templates_fetch.py)")
    if not os.path.exists(DOCX_PDF):
        raise SystemExit(f"変換器がありません: {DOCX_PDF}\n"
                         "cargo build --release -p paper --example docx_pdf")

    rows = read_mokuroku(mokuroku)
    if a.only:
        rows = [r for r in rows if r[0] == a.only]
    if a.limit:
        rows = rows[:a.limit]

    out = os.path.join(ROOT, "結果.tsv")
    res = []
    for i, c in enumerate(rows, 1):
        guid, kind, title_en, title_ja = c[0], c[1], c[2], c[3]
        for lang, title in (("en", title_en), ("ja", title_ja)):
            try:
                r = one(guid, kind, title, lang, a.skip_word)
            except Tomeru as e:
                # ms_pdf stops when Word will not close its documents.
                print(f"止めます: {e}")
                write(out, res)
                return 1
            except Exception as e:  # noqa: BLE001 - one file must not stop the run
                r = [guid, LOCALE[lang], kind, title, "", "", "", "", str(e).splitlines()[-1][:200]]
            res.append(r)
            print(f"[{i}/{len(rows)}] {LOCALE[lang]} {guid} {kind} "
                  f"Word {r[4] or '-'} / うち {r[5] or '-'} {r[6]} {r[7]} {r[8]}")
            write(out, res)
    n = len(res)
    ok = sum(1 for r in res if r[6] == "○")
    print(f"\n結果: {out}")
    print(f"刷った数 {n}、頁数が合った数 {ok}")
    return 0


def write(path, res):
    head = "GUID\t言語\t種類\t題\tWord の頁数\tうちの頁数\t一致\t行の得点\t失敗\n"
    with open(path, "w", encoding="utf-8") as f:
        f.write(head)
        for r in res:
            f.write("\t".join(r) + "\n")


if __name__ == "__main__":
    sys.exit(main())
