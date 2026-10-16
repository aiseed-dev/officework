#!/usr/bin/env python3
"""**officework の PDF と、Word / Excel が出した PDF を比べる道具。**

同じ docx / xlsx から、officework(`save("x.pdf")`)と Word / Excel
(`tools/ms_pdf.py`)の2つの PDF を作り、ページ数・用紙・行の並びを比べます。
絵の画素は比べません。字の位置(行の y と字の x)を比べます。

    .venv/bin/python tools/ms_compare.py うち.pdf 本家.pdf [--tol 6]

出す物: ページ数と用紙の違い、ページごとの行数の違い、行の字の違い、
同じ字の行の y のずれ(tol pt を超えた物)、同じ字の行の字の色の違い
(CIELAB の ΔE が 6 を超えた物。2026-10-16)。違いが無ければ 0 で終わります。
色の違いの数は、最後の行の前に「色の違い N 件」として別にも出します。

## 見方の決め

* 行は「同じベースライン(±2pt)の字」でまとめ、x で並べて繋ぎます。
  上付き・下付きは別の行に見えます
* 空白は数えません。Word は和文の間に空白を挟まないが、こちらは字の
  塊ごとに出すことがあるためです
* 出す y はベースライン(ページの上からの pt)です
* ページの大きさは pt で、1pt までの違いは同じと見ます
"""
import sys
import unicodedata

# A letter's colour differs when the colours are more than this far apart
# (CIE76 ΔE in CIELAB). The PDF's own colour values are compared, so a thin
# red letter is not washed out by the white around it as in a picture
COLOUR_DE = 6.0


def rgb_of(colour):
    """A pdfplumber fill colour (gray, RGB or CMYK, 0 to 1) as RGB 0 to 255.
    No colour is black, the PDF default"""
    if isinstance(colour, (int, float)):
        colour = (colour,)
    if not isinstance(colour, (tuple, list)) or not all(isinstance(v, (int, float)) for v in colour):
        return (0, 0, 0)
    if len(colour) == 1:
        g = colour[0]
        return (g * 255, g * 255, g * 255)
    if len(colour) == 3:
        return tuple(v * 255 for v in colour)
    if len(colour) == 4:
        c, m, y, k = colour
        return ((1 - c) * (1 - k) * 255, (1 - m) * (1 - k) * 255, (1 - y) * (1 - k) * 255)
    return (0, 0, 0)


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


def delta_e(a, b):
    return sum((u - v) ** 2 for u, v in zip(lab(a), lab(b))) ** 0.5


def hexrgb(rgb):
    return "#%02X%02X%02X" % tuple(int(round(v)) for v in rgb)

import pdfplumber


def lines_of(page, ytol=2.0):
    """行ごとの (ベースライン y, 左端 x, 字) の一覧。

    y は**ベースライン**(字の行列の平行移動の y)で比べます。字の箱の上端
    (`top`)は書体の高さの取り方で変わるので、Word の MS 明朝とこちらの
    ヒラギノで同じ位置の字が 3〜4pt 違って見えました(2026-09-08)。
    ベースラインなら書体が違っても同じ物差しです
    """
    rows = []
    for c in sorted(page.chars, key=lambda c: (-c["matrix"][5], c["x0"])):
        if c["text"].isspace():
            continue
        base = page.height - c["matrix"][5]
        for r in rows:
            if abs(r["top"] - base) <= ytol:
                r["chars"].append(c)
                break
        else:
            rows.append({"top": base, "chars": [c]})
    out = []
    for r in sorted(rows, key=lambda r: r["top"]):
        cs = sorted(r["chars"], key=lambda c: c["x0"])
        # Excel の PDF は「月」を康熙部首の「⽉」(U+2F49)で書き、円記号を
        # バックスラッシュで書く(MS 明朝の JIS の癖)。字の比べでは同じと見る
        text = unicodedata.normalize("NFKC", "".join(c["text"] for c in cs))
        # MS 明朝の全角ハイフン「－」は Word の PDF で U+2212(マイナス)に
        # なる(書体の cmap の癖)。字の比べでは同じと見る
        text = text.replace("\u2212", "-").replace("\u2010", "-").replace("\u2015", "-").replace("\u2014", "-")
        out.append({
            "top": r["top"],
            "x0": cs[0]["x0"],
            "text": text.replace("(cid:0)", "□").replace("\\", "¥"),
            "size": cs[0].get("size"),
            # One colour per letter as drawn, for the colour check
            "colours": [rgb_of(c.get("non_stroking_color")) for c in cs],
        })
    return out


def compare(a_path, b_path, tol=6.0):
    """行を**字で対応付けてから**位置を比べます(2026-09-09)。

    前は何行目どうしを並べて比べていたので、1行ずれると後ろが全部「字が違う」に
    なり、違いの数が本当の違いを表しませんでした。difflib で同じ字の行を
    突き合わせ、片方にしか無い行と、同じ行の y・x のずれを数えます。
    ページごとに対応付け、ページ数が違うときは残りのページを「だけ」に数えます
    """
    import difflib

    diffs = []
    with pdfplumber.open(a_path) as a, pdfplumber.open(b_path) as b:
        if len(a.pages) != len(b.pages):
            diffs.append(f"ページ数: うち {len(a.pages)} / 本家 {len(b.pages)}")
        for i in range(max(len(a.pages), len(b.pages))):
            pa = a.pages[i] if i < len(a.pages) else None
            pb = b.pages[i] if i < len(b.pages) else None
            la = lines_of(pa) if pa else []
            lb = lines_of(pb) if pb else []
            if pa and pb and (abs(pa.width - pb.width) > 1 or abs(pa.height - pb.height) > 1):
                diffs.append(f"p{i + 1} 用紙: うち {pa.width:.0f}x{pa.height:.0f} / 本家 {pb.width:.0f}x{pb.height:.0f}")
            if len(la) != len(lb):
                diffs.append(f"p{i + 1} 行数: うち {len(la)} / 本家 {len(lb)}")
            sm = difflib.SequenceMatcher(None, [x["text"] for x in la], [y["text"] for y in lb], autojunk=False)
            for tag, a0, a1, b0, b1 in sm.get_opcodes():
                if tag == "equal":
                    for j in range(a1 - a0):
                        x, y = la[a0 + j], lb[b0 + j]
                        if abs(x["top"] - y["top"]) > tol:
                            diffs.append(f"p{i + 1} 行{a0 + j + 1} y: うち {x['top']:.1f} / 本家 {y['top']:.1f} 「{x['text'][:20]}」")
                        if abs(x["x0"] - y["x0"]) > tol:
                            diffs.append(f"p{i + 1} 行{a0 + j + 1} x: うち {x['x0']:.1f} / 本家 {y['x0']:.1f} 「{x['text'][:20]}」")
                        # The letters' colours, letter by letter when both
                        # lines have as many, else the most used colour
                        ca, cb = x["colours"], y["colours"]
                        if len(ca) != len(cb):
                            ca, cb = [max(ca, key=ca.count)], [max(cb, key=cb.count)]
                        for u, v in zip(ca, cb):
                            if delta_e(u, v) > COLOUR_DE:
                                diffs.append(f"p{i + 1} 行{a0 + j + 1} 色: うち {hexrgb(u)} / 本家 {hexrgb(v)} 「{x['text'][:20]}」")
                                break
                    continue
                for j in range(a0, a1):
                    diffs.append(f"p{i + 1} 行{j + 1} うちだけ: 「{la[j]['text'][:40]}」 y={la[j]['top']:.1f}")
                for j in range(b0, b1):
                    diffs.append(f"p{i + 1} 行{j + 1} 本家だけ: 「{lb[j]['text'][:40]}」 y={lb[j]['top']:.1f}")
    return diffs


if __name__ == "__main__":
    args = [x for x in sys.argv[1:] if not x.startswith("--")]
    tol = 6.0
    if "--tol" in sys.argv:
        tol = float(sys.argv[sys.argv.index("--tol") + 1])
        args = [x for x in args if x != str(tol) and x != sys.argv[sys.argv.index("--tol") + 1]]
    if len(args) != 2:
        raise SystemExit(__doc__)
    d = compare(args[0], args[1], tol)
    for line in d:
        print(line)
    print(f"色の違い {sum(' 色: ' in x for x in d)} 件")
    print(f"違い {len(d)} 件")
    sys.exit(1 if d else 0)
