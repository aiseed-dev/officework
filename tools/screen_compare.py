#!/usr/bin/env python3
"""Compare our pages of a docx or an xlsx with Word's or Excel's PDF of it,
as pictures.

    DYLD_FALLBACK_LIBRARY_PATH=.venv/lib .venv/bin/python tools/screen_compare.py \\
        文書.docx Word.pdf [--dpi 100] [--mm 0.5] [--tol 10] [--out 出力先] [--max 2.5]
        [--platform mac]

Our pages come from `Doc.save("x.png")` for a docx, which draws the pages
the PDF prints (`paper::doc_pages`), and from `Book.save("x.png")` for an
xlsx. Word's PDF is drawn at the same resolution
with pypdfium2. Build the Python engine again after changing the engine,
or this compares the old one.

For each page it prints the paper sizes, the share of the page that
differs, and the largest places that differ (in mm from the top left of the
page). A pixel differs when no pixel of the other picture within `--mm`
has a colour within `--tol` (0-255, in every channel), so a shift under
that distance and the edges of letters do not count, while a pale shading
that is there on one side only does. Pages of different sizes are compared
over the larger size, the missing part counted as differing.

With `--out`, each page is written as three pictures side by side: ours,
Word's, and the two laid over each other (red: only in Word, blue: only in
ours, purple: both differ there).

This compares pictures, not text; `tools/ms_compare.py` compares the lines
of text of two PDFs.

Exit status: 0 when it ran; 1 with `--max` when the page counts or paper
sizes differ or a page differs by more than that percentage; 2 for a
usage error or when a file cannot be read or drawn.
"""
import argparse
import os
import sys
import tempfile

import numpy as np
import pypdfium2
from PIL import Image

# A cell of the grid the places are found on (mm), and the share of its
# pixels that must differ for the cell to count
CELL_MM = 2.0
CELL_SHARE = 0.15


def ours_pictures(src, dpi, work, platform=None):
    """The paths of our page pictures, in page order. `platform` is the
    Excel that made an xlsx ("windows" or "mac"; see Book.open)."""
    first = os.path.join(work, "ow.png")
    if src.lower().endswith(".xlsx"):
        from officework import sheet

        sheet.Book.open(src, platform=platform).save(first, dpi=dpi)
    else:
        from officework import doc

        doc.Doc.open(src).save(first, dpi=dpi)
    out, k = [], 1
    while True:
        name = first if k == 1 else os.path.join(work, f"ow-{k}.png")
        if not os.path.exists(name):
            return out
        out.append(name)
        k += 1


def unmatched(a, b, r, tol):
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
            found |= np.abs(ai - s).max(axis=2) <= tol
    return ~found


def places(mask_a, mask_b, px_mm, limit=8):
    """The largest places that differ, as (x, y, w, h in mm, pixels only in
    ours, pixels only in Word's), largest first."""
    h, w = mask_a.shape
    c = max(1, int(round(CELL_MM * px_mm)))
    gh, gw = (h + c - 1) // c, (w + c - 1) // c
    pad = np.zeros((gh * c, gw * c), dtype=bool)
    pad[:h, :w] = mask_a | mask_b
    # The share is of the pixels of the cell that are on the page, so a
    # cell cut by the edge of the page counts the same as a whole one
    inside = np.zeros_like(pad)
    inside[:h, :w] = True
    count = pad.reshape(gh, c, gw, c).sum(axis=(1, 3))
    area = inside.reshape(gh, c, gw, c).sum(axis=(1, 3))
    cells = count >= CELL_SHARE * np.maximum(area, 1)
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
    o[mask_b & ~mask_a] = (220, 30, 30)
    o[mask_a & ~mask_b] = (30, 60, 220)
    o[mask_a & mask_b] = (150, 40, 170)
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


def pad_to(a, h, w):
    """`a` on white paper of h × w pixels, and where it had no paper."""
    out = np.full((h, w, 3), 255, dtype=np.uint8)
    out[:a.shape[0], :a.shape[1]] = a[:h, :w]
    missing = np.ones((h, w), dtype=bool)
    missing[:a.shape[0], :a.shape[1]] = False
    return out, missing


def run(args):
    px_mm = args.dpi / 25.4
    r = max(1, int(round(args.mm * px_mm)))
    if args.out:
        os.makedirs(args.out, exist_ok=True)
    pdf = pypdfium2.PdfDocument(args.pdf)
    bad = False
    with tempfile.TemporaryDirectory() as work:
        ours = ours_pictures(args.source, args.dpi, work, args.platform)
        n_word = len(pdf)
        print(f"頁数: うち {len(ours)}、Word {n_word}")
        bad |= len(ours) != n_word
        # One page at a time, so a long document does not fill the memory
        for k in range(max(len(ours), n_word)):
            if k >= len(ours) or k >= n_word:
                print(f"頁 {k + 1}: {'うち' if k >= len(ours) else 'Word'}にありません")
                continue
            a = np.asarray(Image.open(ours[k]).convert("RGB"))
            b = np.asarray(pdf[k].render(scale=args.dpi / 72.0).to_pil().convert("RGB"))
            size_a = (a.shape[1] / px_mm, a.shape[0] / px_mm)
            size_b = (b.shape[1] / px_mm, b.shape[0] / px_mm)
            line = f"頁 {k + 1}: 紙 {size_a[0]:.0f}×{size_a[1]:.0f}mm"
            # Our pictures round the size and pypdfium2 rounds it up, so a
            # pixel either way is the same paper
            near = max(1.0, 1.5 / px_mm)
            if abs(size_a[0] - size_b[0]) > near or abs(size_a[1] - size_b[1]) > near:
                line += f"(Word は {size_b[0]:.0f}×{size_b[1]:.0f}mm)"
                bad = True
            h, w = max(a.shape[0], b.shape[0]), max(a.shape[1], b.shape[1])
            a, gone_a = pad_to(a, h, w)
            b, gone_b = pad_to(b, h, w)
            mask_a = unmatched(a, b, r, args.tol) | gone_b
            mask_b = unmatched(b, a, r, args.tol) | gone_a
            share = (mask_a | mask_b).mean() * 100
            line += (f"、違う所 {share:.1f}%(うちだけ {mask_a.mean() * 100:.1f}%、"
                     f"Word だけ {mask_b.mean() * 100:.1f}%)")
            print(line)
            if args.max is not None and share > args.max:
                bad = True
            for x, y, pw, ph, na, nb in places(mask_a, mask_b, px_mm):
                side = "うちだけ" if na > 2 * nb else "Word だけ" if nb > 2 * na else "両方"
                print(f"    x {x:5.1f} y {y:5.1f} mm、{pw:5.1f}×{ph:5.1f} mm、{side}")
            if args.out:
                side_by_side([a, b, overlay(b, mask_a, mask_b)]).save(
                    os.path.join(args.out, f"page-{k + 1}.png"))
    pdf.close()
    return 1 if (bad and args.max is not None) else 0


def main(argv):
    ap = argparse.ArgumentParser(
        description="Compare our pages of a docx with Word's PDF of it, as pictures.",
        allow_abbrev=False,
    )
    ap.add_argument("source", help="the docx or xlsx")
    ap.add_argument("--platform", choices=["windows", "mac"],
                    help="read an xlsx as the Excel of this platform lays it out")
    ap.add_argument("pdf")
    ap.add_argument("--dpi", type=float, default=100.0)
    ap.add_argument("--mm", type=float, default=0.5, help="distance a pixel may move (mm)")
    ap.add_argument("--tol", type=int, default=10, help="colour difference taken as the same (0-255)")
    ap.add_argument("--out", help="folder for the side-by-side pictures")
    ap.add_argument("--max", type=float, help="exit 1 when a page differs by more (percent)")
    args = ap.parse_args(argv)  # exits 2 on a usage error
    if args.dpi <= 0 or args.mm < 0 or not 0 <= args.tol <= 255:
        ap.error("--dpi must be above 0, --mm at least 0, --tol from 0 to 255")
    try:
        return run(args)
    except Exception as e:  # a file that cannot be read or drawn
        print(f"比べられません: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
