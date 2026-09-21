#!/usr/bin/env python3
"""Make the gallery web page from the template comparison.

Reads `目録.tsv` and `結果.tsv` under `~/Documents/officework-cmp/templates/`,
renders page 1 of Word's PDF and of ours at 60 dpi, and writes a static page:

    web/gallery/<GUID>-<locale>-word.png
    web/gallery/<GUID>-<locale>-ours.png
    web/gallery/index.html

    python3 tools/templates_gallery.py
    python3 tools/templates_gallery.py --no-images

One card per template shows both locales. Cards whose page counts do not agree
come first. `web/gallery/` is not tracked by git: the pictures are Microsoft's
content and the owner decides whether they may be published.
"""
import argparse
import datetime
import html
import glob
import os
import sys

import pdfplumber

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ROOT = os.path.expanduser("~/Documents/officework-cmp/templates")
OUT = os.path.join(REPO, "web", "gallery")
LOCALES = ["en-us", "ja-jp"]
LANG_OF = {"en-us": "en", "ja-jp": "ja"}
PAGE = "https://word.cloud.microsoft/create/{lang}/{kind}-templates/"


def tsv(path):
    rows = []
    for line in open(path, encoding="utf-8").read().splitlines()[1:]:
        if line.strip():
            rows.append(line.split("\t"))
    return rows


def render(pdf_path, png_path, dpi=60):
    """Write page 1 of a PDF as a PNG. Returns False when it cannot be read."""
    if os.path.exists(png_path):
        return True
    try:
        with pdfplumber.open(pdf_path) as pdf:
            if not pdf.pages:
                return False
            pdf.pages[0].to_image(resolution=dpi).original.save(png_path)
        return True
    except Exception:  # noqa: BLE001 - a broken PDF just has no picture
        return False


CSS = """
:root { color-scheme: light; }
body { margin: 0; padding: 16px; font-family: "Hiragino Sans", "Yu Gothic", sans-serif;
       background: #f4f4f6; color: #1b1b1f; }
h1 { font-size: 20px; margin: 0 0 8px; }
.summary { margin: 0 0 16px; font-size: 14px; line-height: 1.7; }
.cards { display: flex; flex-wrap: wrap; gap: 12px; }
.card { background: #fff; border: 1px solid #d8d8dd; border-radius: 8px; padding: 12px;
        width: 420px; max-width: 100%; box-sizing: border-box; }
.card.chigau { border-color: #c4443a; }
.title { font-size: 15px; font-weight: 600; margin: 0 0 2px; }
.title2 { font-size: 13px; color: #555; margin: 0 0 6px; }
.kind { font-size: 12px; color: #666; margin: 0 0 8px; }
.kind a { color: #1a5fb4; }
.locale { margin-top: 10px; }
.locale h3 { font-size: 13px; margin: 0 0 4px; font-weight: 600; }
.pair { display: flex; flex-wrap: wrap; gap: 8px; }
.shot { flex: 1 1 45%; min-width: 120px; }
.shot span { display: block; font-size: 11px; color: #666; margin-bottom: 2px; }
.shot img { width: 100%; height: auto; border: 1px solid #ccc; background: #fff; }
.nashi { font-size: 12px; color: #888; padding: 8px; border: 1px dashed #ccc; }
.maru { color: #1a7f37; font-weight: 600; }
.batsu { color: #c4443a; font-weight: 600; }
.err { font-size: 12px; color: #c4443a; word-break: break-all; }
@media (max-width: 480px) { body { padding: 10px; } .card { width: 100%; } }
"""


def shot(guid, loc, kind, tag, label, have):
    name = f"{guid}-{loc}-{tag}.png"
    if not have:
        return f'<div class="shot"><span>{label}</span><div class="nashi">絵はありません</div></div>'
    return (f'<div class="shot"><span>{label}</span>'
            f'<img loading="lazy" src="{name}" alt="{label}"></div>')



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


def main(argv=None):
    p = argparse.ArgumentParser(description="突き合わせの結果からギャラリーの頁を作ります")
    p.add_argument("--root", default=ROOT, help="置き場")
    p.add_argument("--out", default=OUT, help="書き出し先")
    p.add_argument("--dpi", type=int, default=60, help="絵の細かさ(既定 60)")
    p.add_argument("--no-images", action="store_true", help="絵を作らず HTML だけ書き直します")
    a = p.parse_args(argv)

    root, out = os.path.expanduser(a.root), os.path.expanduser(a.out)
    mokuroku, kekka = os.path.join(root, "目録.tsv"), os.path.join(root, "結果.tsv")
    for f in (mokuroku, kekka):
        if not os.path.exists(f):
            raise SystemExit(f"ありません: {f}")
    os.makedirs(out, exist_ok=True)

    titles = {}
    for c in tsv(mokuroku):
        titles[c[0]] = (c[1], c[2], c[3])  # kind, English title, Japanese title
    res = {}
    for c in tsv(kekka):
        c += [""] * (9 - len(c))
        res[(c[0], c[1])] = c

    n_img = 0
    pics = {}
    for (guid, loc), c in res.items():
        for tag, suffix in (("word", ".ms.pdf"), ("ours", ".ours.pdf")):
            src = base_of(root, loc, guid, c[2]) + suffix
            png = os.path.join(out, f"{guid}-{loc}-{tag}.png")
            if a.no_images:
                pics[(guid, loc, tag)] = os.path.exists(png)
                continue
            ok = os.path.exists(src) and render(src, png, a.dpi)
            pics[(guid, loc, tag)] = ok
            n_img += ok

    # One card per template; the ones whose page counts differ come first.
    guids = sorted({g for g, _ in res})

    def rank(g):
        bad = sum(1 for loc in LOCALES if res.get((g, loc), [""] * 9)[6] == "×")
        err = sum(1 for loc in LOCALES if res.get((g, loc), [""] * 9)[8])
        sc = min([float(res[(g, loc)][7]) for loc in LOCALES
                  if (g, loc) in res and res[(g, loc)][7]] or [1.0])
        return (-bad, -err, sc, g)

    guids.sort(key=rank)

    # matched / total per locale
    tally = []
    for loc in LOCALES:
        rows = [c for (g, l), c in res.items() if l == loc]
        ok = sum(1 for c in rows if c[6] == "○")
        tally.append(f"{loc}: 頁数が合った物 {ok} / {len(rows)}")

    parts = []
    for g in guids:
        kind, t_en, t_ja = titles.get(g, ("", "", ""))
        chigau = any(res.get((g, loc), [""] * 9)[6] == "×" for loc in LOCALES)
        body = []
        for loc in LOCALES:
            c = res.get((g, loc))
            if not c:
                continue
            mark = ('<span class="maru">一致</span>' if c[6] == "○"
                    else '<span class="batsu">不一致</span>' if c[6] == "×" else "—")
            sc = f"、行の得点 {c[7]}" if c[7] else ""
            err = f'<div class="err">{html.escape(c[8])}</div>' if c[8] else ""
            body.append(
                f'<div class="locale"><h3>{loc} — 頁数 Word {c[4] or "—"} / うち {c[5] or "—"} '
                f'{mark}{sc}</h3>{err}<div class="pair">'
                + shot(g, loc, kind, "word", "Word", pics.get((g, loc, "word")))
                + shot(g, loc, kind, "ours", "officework", pics.get((g, loc, "ours")))
                + "</div></div>")
        link_en = PAGE.format(lang="en", kind=kind)
        link_ja = PAGE.format(lang="ja", kind=kind)
        parts.append(
            f'<div class="card{" chigau" if chigau else ""}">'
            f'<p class="title">{html.escape(t_en)}</p>'
            f'<p class="title2">{html.escape(t_ja)}</p>'
            f'<p class="kind">種類: {html.escape(kind)} — '
            f'<a href="{link_en}" target="_blank" rel="noreferrer">Microsoft の頁(英語)</a> / '
            f'<a href="{link_ja}" target="_blank" rel="noreferrer">日本語</a></p>'
            + "".join(body) + "</div>")

    today = datetime.date.today().isoformat()
    doc = f"""<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Word のテンプレートの突き合わせ</title>
<style>{CSS}</style>
</head>
<body>
<h1>Word のテンプレートの突き合わせ({today})</h1>
<p class="summary">Microsoft が公開している Word のテンプレートを、英語版と日本語版の組で
集め、本物の Word で刷った PDF と officework で刷った PDF を並べています。
テンプレートの数 {len(guids)} 組、刷った数 {len(res)} 件。<br>
{"<br>".join(tally)}<br>
頁数の合わない物を先に並べています。絵は各 PDF の 1 頁目です。</p>
<div class="cards">
{"".join(parts)}
</div>
</body>
</html>
"""
    index = os.path.join(out, "index.html")
    with open(index, "w", encoding="utf-8") as f:
        f.write(doc)
    print(f"作った絵 {n_img} 枚")
    print(f"ギャラリー: {index}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
