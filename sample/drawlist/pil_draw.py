"""Draws a draw list (Book.draw_list()) with Pillow, computing nothing.

It is the plainest drawer: every position, size and font comes from the
list, as the Flet component will take them (docs/sekkei/drawlist.ja.adoc).
It is used to check that the list alone gives the same page as the PDF.

    python pil_draw.py drawlist.json out.png [dpi]
"""
import base64
import io
import json
import sys

from PIL import Image, ImageDraw, ImageFont


def color(c, alpha=1.0):
    c = c.lstrip("#")
    return (int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16), round(255 * alpha))


def dashed(d, a, b, width, fill, dash):
    """A straight line cut into dashes (Pillow has no dashes)."""
    (x1, y1), (x2, y2) = a, b
    length = ((x2 - x1) ** 2 + (y2 - y1) ** 2) ** 0.5
    if not dash or length == 0:
        d.line([a, b], fill=fill, width=width)
        return
    ux, uy = (x2 - x1) / length, (y2 - y1) / length
    pos, i = 0.0, 0
    while pos < length:
        seg = dash[i % len(dash)]
        if i % 2 == 0:
            end = min(pos + seg, length)
            d.line([(x1 + ux * pos, y1 + uy * pos), (x1 + ux * end, y1 + uy * end)], fill=fill, width=width)
        pos += seg
        i += 1


def path_points(d_list, k, steps=12):
    """The polygons of a path (curves as short lines), one per M."""
    polys, cur, last = [], [], (0, 0)
    for seg in d_list:
        op = seg[0]
        if op == "M":
            if cur:
                polys.append(cur)
            last = (seg[1] * k, seg[2] * k)
            cur = [last]
        elif op == "L":
            last = (seg[1] * k, seg[2] * k)
            cur.append(last)
        elif op == "C":
            x0, y0 = last
            x1, y1, x2, y2, x3, y3 = [v * k for v in seg[1:]]
            for s in range(1, steps + 1):
                t = s / steps
                u = 1 - t
                cur.append((u**3 * x0 + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t**3 * x3,
                            u**3 * y0 + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t**3 * y3))
            last = (x3, y3)
        elif op == "Z" and cur:
            cur.append(cur[0])
    if cur:
        polys.append(cur)
    return polys


def draw_page(page, fonts, dpi):
    k = dpi / 72.0
    w, h = page["size"]
    im = Image.new("RGBA", (round(w * k), round(h * k)), (255, 255, 255, 255))
    d = ImageDraw.Draw(im, "RGBA")
    faces = {}

    def face(fid, size):
        key = (fid, size)
        if key not in faces:
            f = fonts[fid]
            faces[key] = ImageFont.truetype(f["file"], max(1.0, size * k), index=f.get("index", 0))
        return faces[key]

    for it in page["items"]:
        t = it["type"]
        if t == "fill":
            x, y, rw, rh = [v * k for v in it["rect"]]
            d.rectangle([x, y, x + rw, y + rh], fill=color(it["color"], it.get("alpha", 1)))
        elif t == "line":
            width = max(1, round(it["width"] * k))
            dashed(d, (it["from"][0] * k, it["from"][1] * k), (it["to"][0] * k, it["to"][1] * k),
                   width, color(it["color"], it.get("alpha", 1)), [v * k for v in it.get("dash", [])])
        elif t == "path":
            for poly in path_points(it["d"], k):
                if it.get("fill") and len(poly) > 2:
                    d.polygon(poly, fill=color(it["fill"], it.get("alpha", 1)))
                if it.get("stroke"):
                    width = max(1, round(it["width"] * k))
                    dash = [v * k for v in it.get("dash", [])]
                    for a, b in zip(poly, poly[1:]):
                        dashed(d, a, b, width, color(it["stroke"], it.get("alpha", 1)), dash)
        elif t == "image":
            x, y, rw, rh = [v * k for v in it["rect"]]
            pic = Image.open(io.BytesIO(base64.b64decode(it["base64"]))).convert("RGBA")
            pic = pic.resize((max(1, round(rw)), max(1, round(rh))))
            if it.get("clip"):
                # A picture stretched past its shape is cut to the shape's box
                cx, cy, cw, ch = [v * k for v in it["clip"]]
                l, t_, r, b = max(x, cx), max(y, cy), min(x + rw, cx + cw), min(y + rh, cy + ch)
                if r <= l or b <= t_:
                    continue
                pic = pic.crop((round(l - x), round(t_ - y), round(r - x), round(b - y)))
                x, y = l, t_
            im.alpha_composite(pic, (round(x), round(y)))
        elif t == "text":
            f = face(it["font"], it["size"])
            d.text((it["x"] * k, it["baseline"] * k), it["text"], font=f, fill=color(it["color"]),
                   anchor="ls", stroke_width=1 if it.get("bold") else 0,
                   stroke_fill=color(it["color"]) if it.get("bold") else None)
    return im.convert("RGB")


def main():
    src, out = sys.argv[1], sys.argv[2]
    dpi = float(sys.argv[3]) if len(sys.argv) > 3 else 96
    dl = json.load(open(src, encoding="utf-8"))
    fonts = {f["id"]: f for f in dl["fonts"]}
    for i, page in enumerate(dl["pages"]):
        name = out if i == 0 else out.replace(".png", f"-{i + 1}.png")
        draw_page(page, fonts, dpi).save(name)
        print(name)


if __name__ == "__main__":
    main()
