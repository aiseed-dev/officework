#!/usr/bin/env python3
"""Compare the ruled lines (borders, underlines drawn as lines, frames) of two PDFs.

    .venv/bin/python tools/ms_rules.py ours.pdf theirs.pdf [--pos 1.5] [--width 0.3] [--de 10]

`tools/ms_compare.py` compares letters and `tools/ms_imgdiff.py` pictures;
neither tells a border that is too thick or of another colour from one that
is in place (2026-10-16). This reads the drawing itself (pdfplumber) and turns
what is drawn into straight horizontal and vertical segments:

* a stroked line, with its line width and stroke colour,
* a thin filled rectangle (under 2pt across), which is how Word and Excel draw
  many borders, with its short side as the width and its fill colour,
* the edges of a stroked rectangle.

Segments of one page are matched by orientation, by position (within `--pos`
pt) and by overlap (half of the shorter one). It reports segments only one
side has, and matched ones whose widths differ by more than `--width` pt or
whose colours differ by more than `--de` (CIE76 ΔE). The last line is the
number of differences.
"""
import sys

import pdfplumber

sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from ms_compare import delta_e, hexrgb, rgb_of  # noqa: E402

THIN = 2.0  # a filled rectangle thinner than this is a rule
MIN_LEN = 3.0  # shorter segments are dots and ticks, not rules


def segments(page):
    """[(orientation 'h'|'v', position, start, end, width, rgb)] in pt from the top left."""
    out = []

    def add(o, pos, a, b, w, rgb):
        a, b = min(a, b), max(a, b)
        if b - a >= MIN_LEN:
            out.append((o, round(pos, 2), a, b, w, rgb))

    for ln in page.lines:
        x0, x1, t, btm = ln["x0"], ln["x1"], ln["top"], ln["bottom"]
        w = ln.get("linewidth") or 1.0
        rgb = rgb_of(ln.get("stroking_color"))
        if abs(t - btm) < 0.5:
            add("h", t, x0, x1, w, rgb)
        elif abs(x0 - x1) < 0.5:
            add("v", x0, t, btm, w, rgb)
    for r in page.rects:
        x0, x1, t, btm = r["x0"], r["x1"], r["top"], r["bottom"]
        wd, ht = x1 - x0, btm - t
        if r.get("fill") and min(wd, ht) < THIN:
            rgb = rgb_of(r.get("non_stroking_color"))
            if wd >= ht:
                add("h", (t + btm) / 2, x0, x1, ht, rgb)
            else:
                add("v", (x0 + x1) / 2, t, btm, wd, rgb)
        elif r.get("stroke"):
            w = r.get("linewidth") or 1.0
            rgb = rgb_of(r.get("stroking_color"))
            add("h", t, x0, x1, w, rgb)
            add("h", btm, x0, x1, w, rgb)
            add("v", x0, t, btm, w, rgb)
            add("v", x1, t, btm, w, rgb)
    return inside_dropped(merge(out))


def inside_dropped(segs):
    """Drop a hairline that lies inside a wider rule along it: Excel draws a
    border as a filled 0.96pt rectangle and a 0.14pt stroke within it, one
    rule to the eye (Excel 16.0.20430, 2026-10-16)"""
    keep = []
    for s in segs:
        o, pos, a, b, w, _ = s
        inside = any(t is not s and t[0] == o and t[4] > w and abs(t[1] - pos) <= t[4] / 2 + 0.2
                     and t[2] <= a + 0.5 and t[3] >= b - 0.5 for t in segs)
        if not inside:
            keep.append(s)
    return keep


def merge(segs):
    """Join collinear touching pieces of the same width and colour (a border
    drawn cell by cell is one rule across the table)."""
    segs = sorted(segs, key=lambda s: (s[0], s[1], s[2]))
    out = []
    for s in segs:
        if out:
            o, pos, a, b, w, rgb = out[-1]
            if (s[0] == o and abs(s[1] - pos) < 0.3 and s[2] <= b + 0.5
                    and abs(s[4] - w) < 0.1 and delta_e(s[5], rgb) < 3):
                out[-1] = (o, pos, a, max(b, s[3]), w, rgb)
                continue
        out.append(s)
    return out


def compare(a_path, b_path, pos_tol=1.5, width_tol=0.3, de=10.0):
    diffs = []
    with pdfplumber.open(a_path) as a, pdfplumber.open(b_path) as b:
        for i in range(min(len(a.pages), len(b.pages))):
            sa, sb = segments(a.pages[i]), segments(b.pages[i])
            used = set()
            for t in sb:
                best, best_k = None, None
                for k, s in enumerate(sa):
                    if k in used or s[0] != t[0] or abs(s[1] - t[1]) > pos_tol:
                        continue
                    overlap = min(s[3], t[3]) - max(s[2], t[2])
                    if overlap < 0.5 * min(s[3] - s[2], t[3] - t[2]):
                        continue
                    score = abs(s[1] - t[1]) + abs(s[2] - t[2]) + abs(s[3] - t[3]) + 4 * abs(s[4] - t[4])
                    if best is None or score < best:
                        best, best_k = score, k
                where = f"{'横' if t[0] == 'h' else '縦'} {t[1]:.1f} の {t[2]:.0f}〜{t[3]:.0f}"
                if best_k is None:
                    diffs.append(f"p{i + 1} 本家だけの罫線: {where} 太さ {t[4]:.2f} {hexrgb(t[5])}")
                    continue
                used.add(best_k)
                s = sa[best_k]
                if abs(s[4] - t[4]) > width_tol:
                    diffs.append(f"p{i + 1} 罫線の太さ: {where} うち {s[4]:.2f} / 本家 {t[4]:.2f}")
                if delta_e(s[5], t[5]) > de:
                    diffs.append(f"p{i + 1} 罫線の色: {where} うち {hexrgb(s[5])} / 本家 {hexrgb(t[5])}")
            for k, s in enumerate(sa):
                if k not in used:
                    where = f"{'横' if s[0] == 'h' else '縦'} {s[1]:.1f} の {s[2]:.0f}〜{s[3]:.0f}"
                    diffs.append(f"p{i + 1} うちだけの罫線: {where} 太さ {s[4]:.2f} {hexrgb(s[5])}")
    return diffs


def main():
    args = sys.argv[1:]
    opt = {"--pos": "1.5", "--width": "0.3", "--de": "10"}
    for k in list(opt):
        if k in args:
            i = args.index(k)
            opt[k] = args[i + 1]
            del args[i:i + 2]
    if len(args) != 2:
        raise SystemExit(__doc__)
    d = compare(args[0], args[1], float(opt["--pos"]), float(opt["--width"]), float(opt["--de"]))
    for line in d:
        print(line)
    print(f"罫線の違い {len(d)} 件")
    sys.exit(1 if d else 0)


if __name__ == "__main__":
    main()
