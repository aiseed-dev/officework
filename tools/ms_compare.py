#!/usr/bin/env python3
"""**officework の PDF と、Word / Excel が出した PDF を比べる道具。**

同じ docx / xlsx から、officework(`save("x.pdf")`)と Word / Excel
(`tools/ms_pdf.py`)の2つの PDF を作り、ページ数・用紙・行の並びを比べます。
絵の画素は比べません。字の位置(行の y と字の x)を比べます。

    .venv/bin/python tools/ms_compare.py うち.pdf 本家.pdf [--tol 6]

出す物: ページ数と用紙の違い、ページごとの行数の違い、行の字の違い、
同じ字の行の y のずれ(tol pt を超えた物)。違いが無ければ 0 で終わります。

## 見方の決め

* 行は「同じベースライン(±2pt)の字」でまとめ、x で並べて繋ぎます。
  上付き・下付きは別の行に見えます
* 空白は数えません。Word は和文の間に空白を挟まないが、こちらは字の
  塊ごとに出すことがあるためです
* 出す y はベースライン(ページの上からの pt)です
* ページの大きさは pt で、1pt までの違いは同じと見ます
"""
import sys

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
        out.append({
            "top": r["top"],
            "x0": cs[0]["x0"],
            "text": "".join(c["text"] for c in cs).replace("(cid:0)", "□"),
            "size": cs[0].get("size"),
        })
    return out


def compare(a_path, b_path, tol=6.0):
    diffs = []
    with pdfplumber.open(a_path) as a, pdfplumber.open(b_path) as b:
        if len(a.pages) != len(b.pages):
            diffs.append(f"ページ数: うち {len(a.pages)} / 本家 {len(b.pages)}")
        for i, (pa, pb) in enumerate(zip(a.pages, b.pages), 1):
            if abs(pa.width - pb.width) > 1 or abs(pa.height - pb.height) > 1:
                diffs.append(f"p{i} 用紙: うち {pa.width:.0f}x{pa.height:.0f} / 本家 {pb.width:.0f}x{pb.height:.0f}")
            la, lb = lines_of(pa), lines_of(pb)
            if len(la) != len(lb):
                diffs.append(f"p{i} 行数: うち {len(la)} / 本家 {len(lb)}")
            for j, (x, y) in enumerate(zip(la, lb), 1):
                if x["text"] != y["text"]:
                    diffs.append(f"p{i} 行{j} 字: うち「{x['text'][:40]}」/ 本家「{y['text'][:40]}」")
                    continue
                if abs(x["top"] - y["top"]) > tol:
                    diffs.append(f"p{i} 行{j} y: うち {x['top']:.1f} / 本家 {y['top']:.1f} 「{x['text'][:20]}」")
                if abs(x["x0"] - y["x0"]) > tol:
                    diffs.append(f"p{i} 行{j} x: うち {x['x0']:.1f} / 本家 {y['x0']:.1f} 「{x['text'][:20]}」")
            for j in range(min(len(la), len(lb)), max(len(la), len(lb))):
                side, line = ("うち", la[j]) if j < len(la) else ("本家", lb[j])
                diffs.append(f"p{i} 行{j + 1} {side}だけ: 「{line['text'][:40]}」 y={line['top']:.1f}")
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
    print(f"違い {len(d)} 件")
    sys.exit(1 if d else 0)
