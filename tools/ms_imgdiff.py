#!/usr/bin/env python3
"""Compare two PDFs as pictures: where do their colours differ?

    .venv/bin/python tools/ms_imgdiff.py ours.pdf theirs.pdf [--dpi 50] [--de 12] [--flat-de 4] [--out diff.png]

`tools/ms_compare.py` compares where the letters stand. It does not see
fills, shapes, pictures or what covers what: on 2026-10-15 a merged cell's
gradient banner was missing from officework's PDF and the line comparison
still said 0 (seen beside ibis-office's PDF, which compares pictures).

Each page is drawn at `--dpi` (pdftoppm) and cut into 8-pixel squares. A
square differs when the mean colours of the two pages there are more than
`--de` apart (CIE76 ΔE in CIELAB). Means over a square smooth away the
small differences of a substitute face, so what is left is fills, lines and
pictures. A square that is one flat colour on both pages (a fill, such as a
table's pale stripe) differs already at `--flat-de` (4): a mean over letters
needs the wide margin, a fill does not, and pale fills told apart by less than
12 were missed (2026-10-16). Letter colours are checked by ms_compare.py.
The last line is the number of differing squares; `--out` writes the
reference page with the differing squares outlined in red.
"""
import os
import subprocess
import sys
import tempfile

from PIL import Image, ImageDraw

SQUARE = 8
# A square is flat when no channel varies by more than this inside it
FLAT_SPREAD = 10


def pages(pdf, dpi, tmp, tag):
    subprocess.run(["pdftoppm", "-r", str(dpi), "-png", pdf, os.path.join(tmp, tag)], check=True)
    names = sorted(n for n in os.listdir(tmp) if n.startswith(tag + "-"))
    return [Image.open(os.path.join(tmp, n)).convert("RGB") for n in names]


def lab(rgb):
    def lin(c):
        c /= 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (lin(float(v)) for v in rgb)
    x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047
    y = 0.2126 * r + 0.7152 * g + 0.0722 * b
    z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883

    def f(t):
        return t ** (1 / 3) if t > 0.008856 else 7.787 * t + 16 / 116
    fx, fy, fz = f(x), f(y), f(z)
    return 116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)


def flat(im, i, j):
    box = (i * SQUARE, j * SQUARE, i * SQUARE + SQUARE, j * SQUARE + SQUARE)
    return all(hi - lo <= FLAT_SPREAD for lo, hi in im.crop(box).getextrema())


def compare(a, b, de, flat_de=4.0):
    """The squares (column, row) whose mean colours differ by more than de,
    or by more than flat_de where both squares are one flat colour."""
    w, h = min(a.width, b.width) // SQUARE, min(a.height, b.height) // SQUARE
    ma = a.crop((0, 0, w * SQUARE, h * SQUARE)).resize((w, h), Image.BOX)
    mb = b.crop((0, 0, w * SQUARE, h * SQUARE)).resize((w, h), Image.BOX)
    out = []
    for j in range(h):
        for i in range(w):
            pa, pb = ma.getpixel((i, j)), mb.getpixel((i, j))
            if pa == pb:
                continue
            la, lb = lab(pa), lab(pb)
            d = sum((u - v) ** 2 for u, v in zip(la, lb)) ** 0.5
            if d > de or (d > flat_de and flat(a, i, j) and flat(b, i, j)):
                out.append((i, j))
    return out


def main():
    args = sys.argv[1:]
    opt = {"--dpi": "50", "--de": "12", "--flat-de": "4", "--out": None}
    for k in list(opt):
        if k in args:
            i = args.index(k)
            opt[k] = args[i + 1]
            del args[i:i + 2]
    if len(args) != 2:
        raise SystemExit(__doc__)
    dpi, de, flat_de = int(opt["--dpi"]), float(opt["--de"]), float(opt["--flat-de"])
    with tempfile.TemporaryDirectory() as tmp:
        ours, theirs = pages(args[0], dpi, tmp, "a"), pages(args[1], dpi, tmp, "b")
        if len(ours) != len(theirs):
            print(f"ページ数: うち {len(ours)} / 本家 {len(theirs)}")
        total = 0
        marked = []
        for k, (a, b) in enumerate(zip(ours, theirs), 1):
            diff = compare(a, b, de, flat_de)
            total += len(diff)
            if diff:
                print(f"p{k}: 色の違う区画 {len(diff)}")
            pic = b.copy()
            d = ImageDraw.Draw(pic)
            for i, j in diff:
                d.rectangle((i * SQUARE, j * SQUARE, i * SQUARE + SQUARE - 1, j * SQUARE + SQUARE - 1),
                            outline=(230, 0, 0))
            marked.append(pic)
        if opt["--out"] and marked:
            width = max(p.width for p in marked)
            sheet = Image.new("RGB", (width, sum(p.height for p in marked)), "white")
            y = 0
            for p in marked:
                sheet.paste(p, (0, y))
                y += p.height
            sheet.save(opt["--out"])
    print(f"絵の違い {total} 区画")
    sys.exit(0 if total == 0 else 1)


if __name__ == "__main__":
    main()
