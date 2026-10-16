#!/usr/bin/env python3
"""Evaluate how officework's PDF of a document differs from Word's or Excel's,
by what matters to a reader.

    .venv/bin/python tools/pdf_eval.py ours.pdf theirs.pdf [--source doc.docx|book.xlsx] [--json out.json]

What weighs most is what a reader cannot see, then what is shown wrong, then
what is hard to read (decided 2026-10-16). Where things stand and how many
pages there are weigh little, so the comparison is made over the whole
document, not page by page or position by position:

1. Not shown (表示されない)
   * text of theirs that ours lacks (the document's text matched as a whole,
     whatever page it lands on),
   * letters that are in ours but cannot be seen where theirs can (white on
     white, under a picture, cut away): checked on the drawn page, by whether
     the letter's colour shows inside its box,
   * colour on the page: area of a colour theirs has and ours lacks (fills,
     shapes, bands), in square centimetres,
   * ruled lines: length of rules of a colour and weight theirs has and ours
     lacks.
2. Shown wrong (違う)
   * text that differs (classed as capital letters, digits, a missing glyph
     shown as a box, or other), and text only ours has,
   * letters of another colour or another kind of face (Mincho and Gothic),
     and lines drawn in another weight (the ink in the letters' boxes: a
     face made bold by stroking keeps its name, 2026-10-16),
   * colour area only ours has.
3. Hard to read (読みにくい), where theirs is not
   * too little contrast between a letter and what lies under it,
   * letters drawn over other letters,
   * letters much smaller,
   * a line of theirs broken into three or more lines,
   * letters off the paper.

A page break the document asks for and ours ignores weighs heavily too
(改ページ, 2026-10-16): a page break mark, "page break before", a section
that starts on a new page, or a sheet's manual row break. These are read from
`--source`; a break that only moved because a face is wider or narrower is
not counted.

The report lists the findings by weight with examples; `--json` writes them
for `tools/eval_summary.py`, which adds them up over many documents.
"""
import difflib
import json
import os
import subprocess
import sys
import tempfile
import unicodedata
from collections import defaultdict

import pdfplumber
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ms_compare import delta_e, hexrgb, rgb_of  # noqa: E402
from ms_rules import segments  # noqa: E402

DPI = 100
K = DPI / 72.0


def forced_breaks(path):
    """The text that follows each page break the document asks for (its first
    ten letters, normalized as the PDFs' letters are)."""
    import re
    import zipfile

    def first(text):
        t = "".join(norm_char(c) for c in text if not c.isspace())
        return t[:10]
    out = []
    try:
        z = zipfile.ZipFile(path)
    except (OSError, zipfile.BadZipFile):
        return out
    if path.lower().endswith((".docx", ".docm")):
        xml = z.read("word/document.xml").decode("utf-8", "replace")
        body = xml[xml.find("<w:body"):]
        paras = re.findall(r"<w:p[ >].*?</w:p>|<w:p/>", body, re.S)
        pending = False
        for p in paras:
            text_of = lambda frag: "".join(re.findall(r"<w:t(?: [^>]*)?>([^<]*)</w:t>", frag))
            if pending and text_of(p).strip():
                out.append(first(text_of(p)))
                pending = False
            if re.search(r"<w:pageBreakBefore(?: w:val=\"(?:1|true|on)\")?\s*/>", p) and text_of(p).strip():
                out.append(first(text_of(p)))
            for m in re.finditer(r'<w:br [^>]*w:type="page"[^>]*/>', p):
                after = text_of(p[m.end():])
                if after.strip():
                    out.append(first(after))
                else:
                    pending = True
            sect = re.search(r"<w:sectPr.*?</w:sectPr>", p, re.S)
            if sect:
                ty = re.search(r'<w:type w:val="(\w+)"', sect.group(0))
                if not ty or ty.group(1) in ("nextPage", "oddPage", "evenPage"):
                    pending = True
    elif path.lower().endswith((".xlsx", ".xlsm")):
        shared = []
        if "xl/sharedStrings.xml" in z.namelist():
            ss = z.read("xl/sharedStrings.xml").decode("utf-8", "replace")
            shared = ["".join(re.findall(r"<t[^>]*>([^<]*)</t>", si)) for si in re.findall(r"<si>.*?</si>", ss, re.S)]
        for n in z.namelist():
            if not re.match(r"xl/worksheets/sheet\d+\.xml$", n):
                continue
            x = z.read(n).decode("utf-8", "replace")
            rb = re.search(r"<rowBreaks.*?</rowBreaks>", x, re.S)
            if not rb:
                continue
            for brk in re.findall(r'<brk [^>]*id="(\d+)"[^>]*man="(?:1|true)"', rb.group(0)):
                row = int(brk) + 1
                m = re.search(rf'<row [^>]*r="{row}"[^>]*>(.*?)</row>', x, re.S)
                if not m:
                    continue
                texts = []
                for c in re.findall(r'<c [^>]*?(?:t="(\w+)")?[^>]*>(?:<f>.*?</f>)?<v>([^<]*)</v>', m.group(1)):
                    if c[0] == "s" and c[1].isdigit() and int(c[1]) < len(shared):
                        texts.append(shared[int(c[1])])
                    elif c[1]:
                        texts.append(c[1])
                if texts:
                    out.append(first(texts[0]))
    return [t for t in out if t]


def norm_char(t):
    t = unicodedata.normalize("NFKC", t)
    t = t.replace("−", "-").replace("‐", "-").replace("―", "-").replace("—", "-")
    return "□" if t == "(cid:0)" else t.replace("\\", "¥")


def face_kind(name):
    n = (name or "").lower()
    if any(k in n for k in ("mincho", "明朝", "serif", "times", "century", "song", "ming", "cambria", "georgia")):
        if "sans" not in n:
            return "明朝"
    if any(k in n for k in ("gothic", "ゴシック", "sans", "arial", "meiryo", "メイリオ", "helvetica", "calibri", "aptos", "segoe")):
        return "ゴシック"
    return None


def heavy(name):
    n = (name or "").lower()
    return any(k in n for k in ("bold", "heavy", "black", "demi", "-w6", "-w7", "-w8", "-w9"))


# ---- reading a PDF -------------------------------------------------------

def read(pdf, tmp, tag, drop_invisible=False):
    """Everything a reader would see, page by page. With `drop_invisible`,
    letters that cannot be seen are left out (theirs: a reader of Excel's
    print never sees a template's text meant not to be shown)."""
    subprocess.run(["pdftoppm", "-r", str(DPI), "-png", pdf, os.path.join(tmp, tag)], check=True)
    pics = sorted(n for n in os.listdir(tmp) if n.startswith(tag + "-"))
    doc = {"lines": [], "chars": [], "images": [], "rules": [], "pages": []}
    with pdfplumber.open(pdf) as p:
        for pi, page in enumerate(p.pages):
            im = Image.open(os.path.join(tmp, pics[pi])).convert("RGB") if pi < len(pics) else None
            doc["pages"].append((page.width, page.height, im))
            rows = []
            for c in sorted(page.chars, key=lambda c: (-c["matrix"][5], c["x0"])):
                if c["text"].isspace():
                    continue
                base = page.height - c["matrix"][5]
                for r in rows:
                    if abs(r["base"] - base) <= 2.0:
                        r["cs"].append(c)
                        break
                else:
                    rows.append({"base": base, "cs": [c]})
            for r in sorted(rows, key=lambda r: r["base"]):
                cs = sorted(r["cs"], key=lambda c: c["x0"])
                line = {"page": pi, "chars": []}
                for c in cs:
                    ch = {"t": norm_char(c["text"]), "page": pi, "x0": c["x0"], "x1": c["x1"],
                          "top": c["top"], "bottom": c["bottom"], "size": c.get("size") or 0,
                          "font": c.get("fontname", ""), "rgb": rgb_of(c.get("non_stroking_color")),
                          "line": len(doc["lines"])}
                    if drop_invisible and not visible(ch, im)[0]:
                        continue
                    line["chars"].append(ch)
                    doc["chars"].append(ch)
                if not line["chars"]:
                    continue
                line["text"] = "".join(ch["t"] for ch in line["chars"])
                doc["lines"].append(line)
            for img in page.images:
                doc["images"].append({"page": pi, "w": img["x1"] - img["x0"], "h": img["bottom"] - img["top"]})
            for s in segments(page):
                doc["rules"].append(s)
    return doc


def visible(ch, im):
    """Whether a letter shows on the drawn page, the contrast with what lies
    under it, and how much of its box is ink (for the weight)."""
    if im is None:
        return True, 21.0, 0.0
    x0, x1 = int(ch["x0"] * K), int(ch["x1"] * K) + 1
    y0, y1 = int(ch["top"] * K), int(ch["bottom"] * K) + 1
    if x1 <= x0 or y1 <= y0 or x0 < 0 or y0 < 0 or x1 > im.width or y1 > im.height:
        return False, 1.0, 0.0
    px = list(im.crop((x0, y0, x1, y1)).get_flattened_data()) if hasattr(Image.Image, 'get_flattened_data') else list(im.crop((x0, y0, x1, y1)).getdata())
    t = ch["rgb"]

    def dist(p, q):
        return ((p[0] - q[0]) ** 2 + (p[1] - q[1]) ** 2 + (p[2] - q[2]) ** 2) ** 0.5
    # The ground is the commonest colour away from the letter's: a large bold
    # letter covers more of its box than the ground does
    away = [q for q in px if dist(q, t) > 60]
    bg = max(set(away), key=away.count) if away else max(set(px), key=px.count)
    gap = dist(t, bg)
    if gap < 30:
        return False, contrast(t, bg), 0.0  # the letter's colour is the ground's
    # Ink: pixels nearer the letter's colour than halfway to the ground. A
    # thin Mincho stroke at 100 dpi is grey, not the letter's black
    ink = sum(1 for q in px if dist(q, t) < 0.5 * gap)
    # One pixel is enough: a small "、" at 100 dpi has only a few
    return ink >= 1, contrast(t, bg), ink / len(px)


def luminance(rgb):
    def ch(v):
        v /= 255.0
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (ch(float(v)) for v in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b):
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def colour_areas(doc):
    """Area (cm²) of every colour on the pages outside the letters, by colour
    group (colours within ΔE 10 of each other are one)."""
    groups = []  # [rgb, area]
    cm2 = (2.54 / DPI) ** 2
    for pi, (_, _, im) in enumerate(doc["pages"]):
        if im is None:
            continue
        small = im.resize((im.width // 4, im.height // 4), Image.BOX)
        letters = set()
        for ch in doc["chars"]:
            if ch["page"] == pi:
                for x in range(int(ch["x0"] * K) // 4, int(ch["x1"] * K) // 4 + 1):
                    for y in range(int(ch["top"] * K) // 4, int(ch["bottom"] * K) // 4 + 1):
                        letters.add((x, y))
        counts = defaultdict(int)
        for y in range(small.height):
            for x in range(small.width):
                if (x, y) in letters:
                    continue
                q = small.getpixel((x, y))
                if min(q) > 245:
                    continue  # paper
                counts[tuple(v // 8 * 8 for v in q)] += 1
        for q, n in counts.items():
            for g in groups:
                if delta_e(g[0], q) < 10:
                    g[1] += n * 16 * cm2
                    break
            else:
                groups.append([q, n * 16 * cm2])
    return groups


# ---- the evaluation ------------------------------------------------------

def evaluate(ours_pdf, theirs_pdf, source=None):
    with tempfile.TemporaryDirectory() as tmp:
        a, b = read(ours_pdf, tmp, "a"), read(theirs_pdf, tmp, "b", drop_invisible=True)
        f = {"表示されない": defaultdict(list), "改ページ": defaultdict(list),
             "違う": defaultdict(list), "読みにくい": defaultdict(list)}

        # The text as a whole, line by line and then letter by letter
        sa, sb = [x["text"] for x in a["lines"]], [x["text"] for x in b["lines"]]
        pairs = []  # (our char, their char) for the same letter
        sm = difflib.SequenceMatcher(None, sa, sb, autojunk=False)
        for tag, a0, a1, b0, b1 in sm.get_opcodes():
            ca = [c for ln in a["lines"][a0:a1] for c in ln["chars"]]
            cb = [c for ln in b["lines"][b0:b1] for c in ln["chars"]]
            if tag == "equal":
                pairs += list(zip(ca, cb))
                continue
            ta, tb = [c["t"] for c in ca], [c["t"] for c in cb]
            cm = difflib.SequenceMatcher(None, ta, tb, autojunk=False)

            def seen(chars, doc):
                # Only letters a reader can see count as text that is there
                # or missing: Excel prints a template's column of text meant
                # not to be shown (white) on a page of its own
                return "".join(c["t"] for c in chars if visible(c, doc["pages"][c["page"]][2])[0])
            for t2, i0, i1, j0, j1 in cm.get_opcodes():
                if t2 != "equal":
                    ours, theirs = seen(ca[i0:i1], a), seen(cb[j0:j1], b)
                    if not ours and not theirs:
                        continue
                    if t2 == "replace" and not theirs:
                        t2 = "delete"
                    elif t2 == "replace" and not ours:
                        t2 = "insert"
                if t2 == "equal":
                    pairs += list(zip(ca[i0:i1], cb[j0:j1]))
                elif t2 == "delete":
                    f["違う"]["うちにしか無い字"].append(ours)
                elif t2 == "insert":
                    f["表示されない"]["字が無い"].append(theirs)
                else:
                    if ours.lower() == theirs.lower():
                        kind = "大文字と小文字"
                    elif "□" in ours and "□" not in theirs:
                        kind = "字形が無い(□)"
                    elif any(c.isdigit() for c in ours + theirs):
                        kind = "数字・日付"
                    else:
                        kind = "字が違う"
                    f["違う"][kind].append(f"うち「{ours}」/ 本家「{theirs}」")

        # Letter by letter: shown, colour, face, size; the ink per line
        ink = defaultdict(lambda: [0.0, 0.0, 0])
        for ca, cb in pairs:
            vis_a, con_a, ink_a = visible(ca, a["pages"][ca["page"]][2])
            vis_b, con_b, ink_b = visible(cb, b["pages"][cb["page"]][2])
            if vis_a and vis_b:
                e = ink[cb["line"]]
                e[0] += ink_a
                e[1] += ink_b
                e[2] += 1
            ctx = b["lines"][cb["line"]]["text"][:24]
            if vis_b and not vis_a:
                f["表示されない"]["見えない字"].append(f"「{ca['t']}」 in 「{ctx}」")
            elif vis_a and con_b >= 4.5 and con_a < 3.0:
                f["読みにくい"]["地との差が小さい"].append(f"「{ca['t']}」 {con_a:.1f}:1 / 本家 {con_b:.1f}:1 in 「{ctx}」")
            if delta_e(ca["rgb"], cb["rgb"]) > 10:
                f["違う"]["字の色"].append(f"「{ca['t']}」 うち {hexrgb(ca['rgb'])} / 本家 {hexrgb(cb['rgb'])} in 「{ctx}」")
            ka, kb = face_kind(ca["font"]), face_kind(cb["font"])
            if ka and kb and ka != kb:
                f["違う"]["書体の種類"].append(f"「{ca['t']}」 うち {ka} / 本家 {kb} in 「{ctx}」")
            if cb["size"] and ca["size"] < cb["size"] * 0.8:
                f["読みにくい"]["字が小さい"].append(f"「{ca['t']}」 {ca['size']:.1f}pt / 本家 {cb['size']:.1f}pt in 「{ctx}」")

        # Page breaks the document asks for: where theirs starts a page with
        # the text after the break, ours must not keep it on the page of the
        # text before it
        if source:
            ours_of = {id(cb): ca for ca, cb in pairs}
            tb = "".join(c["t"] for c in b["chars"])
            for want in forced_breaks(source):
                k = tb.find(want)
                if k <= 0:
                    continue
                cb, prev = b["chars"][k], b["chars"][k - 1]
                if cb["page"] == prev["page"]:
                    continue  # theirs does not start a page there either
                ca, pa = ours_of.get(id(cb)), None
                for j in range(k - 1, -1, -1):
                    pa = ours_of.get(id(b["chars"][j]))
                    if pa:
                        break
                if ca and pa and ca["page"] == pa["page"]:
                    f["改ページ"]["改ページを無視"].append(f"「{want}」の前(本家 {cb['page'] + 1} 頁の頭、うち {ca['page'] + 1} 頁の途中)")

        # A line drawn much heavier or lighter: bold that is lost or added.
        # A substitute face draws its strokes a little thicker or thinner,
        # so only a large difference over a line counts
        for ln, (ia, ib, n) in ink.items():
            if n >= 2 and ib > 0 and ia > 0 and not 0.6 < ia / ib < 1.6:
                word = "太い" if ia > ib else "細い"
                f["違う"]["字の太さ"].append(f"うちが{word}(線の量 {ia / ib:.2f} 倍) in 「{b['lines'][ln]['text'][:24]}」")

        # Letters of ours off the paper (cut on a printed page), matched or not
        for ca in a["chars"]:
            w, h = a["pages"][ca["page"]][:2]
            if ca["x1"] > w + 0.5 or ca["x0"] < -0.5 or ca["bottom"] > h + 0.5 or ca["top"] < -0.5:
                f["読みにくい"]["紙からはみ出た字"].append(f"「{ca['t']}」 in 「{a['lines'][ca['line']]['text'][:24]}」(うち {ca['page'] + 1} 頁)")

        # A line of theirs spread over three or more of our lines
        spread = defaultdict(set)
        for ca, cb in pairs:
            spread[cb["line"]].add(ca["line"])
        for ln, ours_lines in spread.items():
            if len(ours_lines) >= 3 and len(b["lines"][ln]["text"]) >= 4:
                f["読みにくい"]["細切れの折り返し"].append(f"「{b['lines'][ln]['text'][:24]}」が {len(ours_lines)} 行")

        # Letters drawn over letters (more in ours than in theirs)
        def overlaps(doc):
            n, ex = 0, []
            by_page = defaultdict(list)
            for ch in doc["chars"]:
                by_page[ch["page"]].append(ch)
            for chs in by_page.values():
                chs.sort(key=lambda c: c["x0"])
                for i, c in enumerate(chs):
                    for d in chs[i + 1:]:
                        if d["x0"] >= c["x1"]:
                            break
                        if d["line"] == c["line"]:
                            continue
                        ox = min(c["x1"], d["x1"]) - max(c["x0"], d["x0"])
                        oy = min(c["bottom"], d["bottom"]) - max(c["top"], d["top"])
                        area = (c["x1"] - c["x0"]) * (c["bottom"] - c["top"])
                        if ox > 0 and oy > 0 and area > 0 and ox * oy > 0.3 * area:
                            n += 1
                            if len(ex) < 5:
                                ex.append(f"「{c['t']}」と「{d['t']}」")
            return n, ex
        na, exa = overlaps(a)
        nb, _ = overlaps(b)
        if na > nb:
            f["読みにくい"]["字の重なり"] = exa + [f"(うち {na} 組 / 本家 {nb} 組)"]

        # Colour on the page outside the letters
        ga, gb = colour_areas(a), colour_areas(b)
        for rgb, area in gb:
            have = sum(x[1] for x in ga if delta_e(x[0], rgb) < 10)
            if area >= 1.0 and have < 0.3 * area:
                f["表示されない"]["色の面が無い"].append(f"{hexrgb(rgb)} 本家 {area:.1f}cm² / うち {have:.1f}cm²")
        for rgb, area in ga:
            have = sum(x[1] for x in gb if delta_e(x[0], rgb) < 10)
            if area >= 1.0 and have < 0.3 * area:
                f["違う"]["うちにしか無い色の面"].append(f"{hexrgb(rgb)} うち {area:.1f}cm² / 本家 {have:.1f}cm²")

        # Ruled lines by colour and weight, as total length
        def rule_len(doc):
            out = defaultdict(float)
            for o, pos, s0, s1, w, rgb in doc["rules"]:
                key = (tuple(v // 32 * 32 for v in rgb), "太" if w >= 1.2 else ("中" if w >= 0.6 else "細"))
                out[key] += (s1 - s0) * 2.54 / 72
            return out
        ra, rb = rule_len(a), rule_len(b)
        for key, ln in rb.items():
            if ln >= 2.0 and ra.get(key, 0.0) < 0.5 * ln:
                f["表示されない"]["罫線が無い"].append(f"{hexrgb(key[0])} {key[1]} 本家 {ln:.0f}cm / うち {ra.get(key, 0.0):.0f}cm")

    return {w: {k: v for k, v in kinds.items() if v} for w, kinds in f.items()}


def main():
    args = sys.argv[1:]
    out = source = None
    if "--source" in args:
        i = args.index("--source")
        source = args[i + 1]
        del args[i:i + 2]
    if "--json" in args:
        i = args.index("--json")
        out = args[i + 1]
        del args[i:i + 2]
    if len(args) != 2:
        raise SystemExit(__doc__)
    res = evaluate(args[0], args[1], source)
    n = 0
    for weight in ("表示されない", "改ページ", "違う", "読みにくい"):
        kinds = res.get(weight, {})
        total = sum(len(v) for v in kinds.values())
        n += total
        print(f"■ {weight}: {total}")
        for kind, items in sorted(kinds.items(), key=lambda kv: -len(kv[1])):
            print(f"  {kind}: {len(items)}  例: {' / '.join(items[:3])}")
    if out:
        with open(out, "w", encoding="utf-8") as fh:
            json.dump(res, fh, ensure_ascii=False, indent=1)
    sys.exit(1 if n else 0)


if __name__ == "__main__":
    main()
