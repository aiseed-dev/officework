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
    python3 tools/templates_round.py --skip a60c389b-9052-4d35-bcfe-b7918b8aad5e
    python3 tools/templates_round.py --redo

`--skip-word` reuses the PDFs that are already there and never starts Word. The
result is written to `templates/結果.tsv`. A file that fails is written with its
error text and the run keeps going.

A pair that was already compared is kept as it stands: its row in `結果.tsv`
holds both page counts and both PDFs are newer than the docx. Such a pair is
neither printed nor read again. `--redo` compares every pair again, which is
what to use after the converter has changed.
"""
import argparse
import glob
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
    """Group the characters of a page into lines of (baseline from the top, text).

    The baseline comes from the text matrix. pdfplumber's `top` is the
    baseline minus the font's declared ascent, and Word's PDFs and ours embed
    the same faces with different ascent values, so `top` differed by several
    points on lines whose baselines agreed within 0.4pt (2026-09-19).
    """
    h = page.height
    chars = sorted(page.chars, key=lambda c: (h - c["matrix"][5], c["x0"]))
    out = []
    for c in chars:
        if not c["text"].strip():
            continue
        base = h - c["matrix"][5]
        if out and abs(base - out[-1][0]) <= 1.5:
            out[-1][1].append(c)
        else:
            out.append([base, [c]])
    rows = []
    for base, cs in out:
        text = "".join(x["text"] for x in sorted(cs, key=lambda x: x["x0"]))
        if text.strip():
            rows.append((base, text.strip()))
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


def read_kekka(path):
    """The rows of an earlier run, keyed by (GUID, locale)."""
    old = {}
    if not os.path.exists(path):
        return old
    for line in open(path, encoding="utf-8").read().splitlines()[1:]:
        c = line.split("\t")
        if len(c) >= 9 and c[0].strip():
            old[(c[0], c[1])] = c
    return old


def reusable(row, guid, lang):
    """True when the earlier row can be kept as it stands.

    That asks for both page counts in the row and for both PDFs to be newer
    than the docx, so neither Word nor our converter has to run again.
    """
    if not row or not row[4].strip() or not row[5].strip():
        return False
    base = base_of(ROOT, LOCALE[lang], guid)
    docx = base + ".docx"
    if not os.path.exists(docx):
        return False
    made = os.path.getmtime(docx)
    for pdf in (base + ".ms.pdf", base + ".ours.pdf"):
        if not os.path.exists(pdf) or os.path.getmtime(pdf) <= made:
            return False
    return True


def base_of(root, locale, guid, kind=None):
    """The path stem of one template, without a suffix.

    The en-us files are filed by kind (`en-us/<kind>/<guid>.docx`,
    2026-09-21); the other locales sit directly under the locale folder.
    A `kind` is used when it is known, and the folders are searched when it
    is not.
    """
    if kind:
        p = os.path.join(root, locale, kind, guid)
        if os.path.exists(p + ".docx"):
            return p
    hit = glob.glob(os.path.join(root, locale, "*", guid + ".docx"))
    if hit:
        return hit[0][: -len(".docx")]
    return os.path.join(root, locale, guid)


SKIP_FILE = [os.path.join(ROOT, "飛ばす.txt")]


def one(guid, kind, title, lang, skip_word):
    skip_file = SKIP_FILE[0]
    """One locale of one template. Returns the row for 結果.tsv."""
    base = base_of(ROOT, LOCALE[lang], guid, kind)
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
            # After an `open` times out (-1712), Word opens nothing more and
            # every later `save as` fails with -1708 on a missing document.
            # Going on would only write 130 rows of the same error
            if "-1712" in str(e) or "-1708" in str(e):
                # Remember the file so the next run skips it from the start
                with open(skip_file, "a", encoding="utf-8") as f:
                    f.write(f"{guid}\t{LOCALE[lang]}\t{kind}\t{str(e).splitlines()[-1][:80]}\n")
                raise Tomeru("Word が開きません(" + str(e).splitlines()[-1][:120]
                             + ")。Word を終了して開き直してから、もう一度動かしてください") from e
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
    p.add_argument("--skip", default="", help="飛ばす GUID(コンマ区切り。Word が開けない物)")
    p.add_argument("--skip-file", default=os.path.join(ROOT, "飛ばす.txt"),
                   help="飛ばす GUID を 1 行 1 つで持つファイル(Word が開けなかった物を道具が足します)")
    p.add_argument("--skip-word", action="store_true", help="Word を動かさず、ある PDF を使います")
    p.add_argument("--redo", action="store_true",
                   help="前の結果を使わず、全部を刷り直して比べます(変換器を直した後に使います)")
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
    skip = set(x for x in a.skip.split(",") if x)
    skip_file = os.path.expanduser(a.skip_file)
    SKIP_FILE[0] = skip_file
    if os.path.exists(skip_file):
        with open(skip_file, encoding="utf-8") as f:
            skip |= {ln.split("\t")[0].strip() for ln in f if ln.strip() and not ln.startswith("#")}
    if skip:
        rows = [r for r in rows if r[0] not in skip]
    if a.limit:
        rows = rows[:a.limit]

    out = os.path.join(ROOT, "結果.tsv")
    old = {} if a.redo else read_kekka(out)
    res = []
    for i, c in enumerate(rows, 1):
        guid, kind, title_en, title_ja = c[0], c[1], c[2], c[3]
        for lang, title in (("en", title_en), ("ja", title_ja)):
            keep = old.get((guid, LOCALE[lang]))
            if reusable(keep, guid, lang):
                res.append(keep)
                print(f"[{i}/{len(rows)}] {LOCALE[lang]} {guid} {kind} "
                      f"Word {keep[4] or '-'} / うち {keep[5] or '-'} {keep[6]} {keep[7]} 前の結果")
                continue
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
