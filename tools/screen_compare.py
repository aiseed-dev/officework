#!/usr/bin/env python3
"""Compare the writer's page pictures with Word's PDF, as pictures.

    .venv/bin/python tools/screen_compare.py 文書.docx Word.pdf [--dpi 100] [--mm 0.5] [--out 出力先]

The pictures of our pages come from `Doc.save("x.png")`, which draws the
same pages (`paper::doc_pages`) the writer's screen shows and the PDF
button writes. Word's PDF is drawn at the same resolution with pypdfium2.

For each page it prints the paper sizes, the share of the page that
differs, and the largest places that differ (in mm from the top left of the
page). A pixel differs when no pixel of the other picture within `--mm`
has nearly the same colour, so a shift under that distance and the edges
of letters do not count. With `--out`, each page is written as three
pictures side by side: ours, Word's, and the two laid over each other
(red: only in Word, blue: only in ours).

This compares pictures, not text. `tools/ms_compare.py` compares the lines
of text of two PDFs; use it to find which line moved.

It exits 0. With `--max 2.5`, it exits 1 when a page differs by more than
2.5 percent, or when the page counts differ.
"""
import os
import sys
import tempfile

import numpy as np
import pypdfium2
from PIL import Image

# Two colours are the same when no channel differs by more than this (0-255)
COLOUR_TOL = 60
# A cell of the grid the places are found on (mm), and the share of its
# pixels that must differ for the cell to count
CELL_MM = 2.0
CELL_SHARE = 0.15


def ours_pictures(docx, dpi, work):
    """The pictures of our pages, one array per page."""
    from officework import doc

    first = os.path.join(work, "ow.png")
    doc.Doc.open(docx).save(first, dpi=dpi)
    out, k = [], 1
    while True:
        name = first if k == 1 else os.path.join(work, f"ow-{k}.png")
        if not os.path.exists(name):
            return out
        out.append(np.asarray(Image.open(name).convert("RGB")))
        k += 1


def word_pictures(pdf, dpi):
    """The pictures of Word's pages, one array per page."""
    d = pypdfium2.PdfDocument(pdf)
    out = []
    for i in range(len(d)):
        im = d[i].render(scale=dpi / 72.0).to_pil().convert("RGB")
        out.append(np.asarray(im))
    d.close()
    return out


def unmatched(a, b, r):
    """Pixels of `a` that have no pixel of nearly the same colour in `b`
    within `r` pixels."""
    h, w, _ = a.shape
    pb = np.pad(b, ((r, r), (r, r), (0, 0)), mode="edge").astype(np.int16)
    ai = a.astype(np.int16)
    found = np.zeros((h, w), dtype=bool)
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            if dy * dy + dx * dx > r * r:
                continue
            s = pb[r + dy:r + dy + h, r + dx:r + dx + w]
            found |= np.abs(ai - s).max(axis=2) <= COLOUR_TOL
    return ~found


def places(mask_a, mask_b, px_mm, limit=8):
    """The largest places that differ, as (x, y, w, h in mm, pixels only in
    ours, pixels only in Word's), largest first."""
    h, w = mask_a.shape
    c = max(1, int(round(CELL_MM * px_mm)))
    gh, gw = (h + c - 1) // c, (w + c - 1) // c
    pad = np.zeros((gh * c, gw * c), dtype=bool)
    pad[:h, :w] = mask_a | mask_b
    cells = pad.reshape(gh, c, gw, c).mean(axis=(1, 3)) >= CELL_SHARE
    seen = np.zeros_like(cells)
    out = []
    for y0 in range(gh):
        for x0 in range(gw):
            if not cells[y0, x0] or seen[y0, x0]:
                continue
            stack, ys, xs = [(y0, x0)], [], []
            seen[y0, x0] = True
            while stack:
                y, x = stack.pop()
                ys.append(y)
                xs.append(x)
                for ny, nx in ((y - 1, x), (y + 1, x), (y, x - 1), (y, x + 1)):
                    if 0 <= ny < gh and 0 <= nx < gw and cells[ny, nx] and not seen[ny, nx]:
                        seen[ny, nx] = True
                        stack.append((ny, nx))
            top, left = min(ys) * c, min(xs) * c
            bottom, right = min(h, (max(ys) + 1) * c), min(w, (max(xs) + 1) * c)
            na = int(mask_a[top:bottom, left:right].sum())
            nb = int(mask_b[top:bottom, left:right].sum())
            out.append((len(ys), left / px_mm, top / px_mm, (right - left) / px_mm,
                        (bottom - top) / px_mm, na, nb))
    out.sort(key=lambda p: -p[0])
    return [p[1:] for p in out[:limit]]


def overlay(b, mask_a, mask_b):
    """Word's page, faded, with what differs in colour."""
    grey = (255 - (255 - b.mean(axis=2)) * 0.35).astype(np.uint8)
    o = np.stack([grey] * 3, axis=2)
    o[mask_b] = (220, 30, 30)
    o[mask_a & ~mask_b] = (30, 60, 220)
    return o


def side_by_side(pictures):
    h = max(p.shape[0] for p in pictures)
    gap = 12
    w = sum(p.shape[1] for p in pictures) + gap * (len(pictures) - 1)
    out = Image.new("RGB", (w, h), (120, 120, 120))
    x = 0
    for p in pictures:
        out.paste(Image.fromarray(p), (x, 0))
        x += p.shape[1] + gap
    return out


def main(argv):
    args, opts, i = [], {}, 0
    while i < len(argv):
        if argv[i].startswith("--") and i + 1 < len(argv):
            opts[argv[i][2:]] = argv[i + 1]
            i += 2
        else:
            args.append(argv[i])
            i += 1
    if len(args) != 2:
        print(__doc__)
        return 2
    docx, pdf = args
    dpi = float(opts.get("dpi", 100))
    px_mm = dpi / 25.4
    r = max(1, int(round(float(opts.get("mm", 0.5)) * px_mm)))
    out_dir = opts.get("out")
    limit = float(opts["max"]) if "max" in opts else None
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)

    with tempfile.TemporaryDirectory() as work:
        ours = ours_pictures(docx, dpi, work)
    word = word_pictures(pdf, dpi)
    bad = len(ours) != len(word)
    print(f"頁数: うち {len(ours)}、Word {len(word)}")
    for k in range(max(len(ours), len(word))):
        if k >= len(ours) or k >= len(word):
            print(f"頁 {k + 1}: {'うち' if k >= len(ours) else 'Word'}にありません")
            continue
        a, b = ours[k], word[k]
        size_a = (a.shape[1] / px_mm, a.shape[0] / px_mm)
        size_b = (b.shape[1] / px_mm, b.shape[0] / px_mm)
        line = f"頁 {k + 1}: 紙 {size_a[0]:.0f}×{size_a[1]:.0f}mm"
        if abs(size_a[0] - size_b[0]) > 1 or abs(size_a[1] - size_b[1]) > 1:
            line += f"(Word は {size_b[0]:.0f}×{size_b[1]:.0f}mm)"
            bad = True
        h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
        a, b = a[:h, :w], b[:h, :w]
        mask_a = unmatched(a, b, r)
        mask_b = unmatched(b, a, r)
        share = (mask_a | mask_b).mean() * 100
        line += (f"、違う所 {share:.1f}%(うちだけ {mask_a.mean() * 100:.1f}%、"
                 f"Word だけ {mask_b.mean() * 100:.1f}%)")
        print(line)
        if limit is not None and share > limit:
            bad = True
        for x, y, pw, ph, na, nb in places(mask_a, mask_b, px_mm):
            side = "うちだけ" if na > 2 * nb else "Word だけ" if nb > 2 * na else "両方"
            print(f"    x {x:5.1f} y {y:5.1f} mm、{pw:5.1f}×{ph:5.1f} mm、{side}")
        if out_dir:
            side_by_side([a, b, overlay(b, mask_a, mask_b)]).save(
                os.path.join(out_dir, f"page-{k + 1}.png"))
    return 1 if (bad and limit is not None) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
