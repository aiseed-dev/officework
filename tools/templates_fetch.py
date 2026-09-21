#!/usr/bin/env python3
"""Collect Microsoft Word's public templates as English/Japanese pairs.

Microsoft publishes the same gallery that Word's "New" screen shows at
`https://word.cloud.microsoft/create/<en|ja>/<kind>-templates/`. Every card on
that page carries a "view in Word" link whose `src` is the docx URL on the
public CDN. The same template has the same GUID in both locales, so a pair can
be collected by keeping the GUIDs that appear on both pages.

    python3 tools/templates_fetch.py
    python3 tools/templates_fetch.py --kinds resume,memo --per-kind 5
    python3 tools/templates_fetch.py --limit 20 --force

The files land under `~/Documents/officework-cmp/templates/` (outside git; the
templates are Microsoft's content and are used only for testing):

    templates/目録.tsv          GUID, kind, English title, Japanese title,
                                en URL, ja URL, date
    templates/en-us/<GUID>.docx English
    templates/ja-jp/<GUID>.docx Japanese

Requests are made one per second with a browser User-Agent. A docx already on
disk is skipped unless `--force` is given. A kind whose page does not exist is
skipped with a note.
"""
import argparse
import datetime
import html
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

UA = ("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")

# The kinds that exist in both en and ja, read from
# https://word.cloud.microsoft/create/sitemap.xml on 2026-09-19. The names in
# the design doc that do not exist (letter, report, invoice, calendar,
# certificate, cv and so on) redirect to the hub page.
KINDS = [
    "resume", "cover-letter", "ats", "letters", "memo", "minutes", "agenda",
    "business", "business-plan", "business-card", "email-signature",
    "fax-cover", "brochure", "pamphlet", "booklet", "flyer", "newsletter",
    "card", "birthday-card", "invitation", "postcard", "label", "menu",
    "wedding", "mothers-day", "learning", "teacher-communication", "writing",
    # Pages whose address has no "-templates" (2026-09-19): the reports
    # live here, 8 in each language
    "papers-and-reports", "meeting-minutes", "meeting-agendas",
]

ROOT = os.path.expanduser("~/Documents/officework-cmp/templates")
LOCALE = {"en": "en-us", "ja": "ja-jp"}
PAGE = "https://word.cloud.microsoft/create/{lang}/{kind}-templates/"
# A kind that already names its page in full ("papers-and-reports")
PAGE_ASIS = "https://word.cloud.microsoft/create/{lang}/{kind}/"


def page_url(kind, lang):
    return (PAGE_ASIS if "-" in kind and kind.endswith(("-reports", "-minutes", "-agendas")) else PAGE).format(lang=lang, kind=kind)

_last = [0.0]


def get(url, timeout=120):
    """Fetch a URL, keeping at least one second between requests."""
    wait = 1.0 - (time.time() - _last[0])
    if wait > 0:
        time.sleep(wait)
    _last[0] = time.time()
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.geturl(), r.read()


CARD = 'data-testid="office-template-grid-card"'
SRC = re.compile(r'view\.aspx\?src=([^"&]+)')
TITLE = re.compile(r'__Title"[^>]*>(.*?)</span>', re.S)
GUID = re.compile(r"/catalog-assets/[a-z-]+/([0-9a-f-]{36})/")
TAG = re.compile(r"<[^>]+>")


def cards(page):
    """Return [(GUID, title, docx URL)] in the order the page shows them."""
    # The parameter used to be named `html`, which hid the `html` module
    # that `html.unescape` needs below (2026-09-19)
    out, seen = [], set()
    for block in page.split(CARD)[1:]:
        m = SRC.search(block)
        if not m:
            continue
        url = urllib.parse.unquote(m.group(1))
        g = GUID.search(url)
        if not g or g.group(1) in seen:
            continue
        t = TITLE.search(block)
        title = html.unescape(TAG.sub("", t.group(1))).strip() if t else ""
        title = re.sub(r"\s+", " ", title).replace("\t", " ")
        seen.add(g.group(1))
        out.append((g.group(1), title, url))
    return out


def one_kind(kind, lang):
    """Read one category page. Returns None when the page does not exist."""
    url = page_url(kind, lang)
    try:
        final, body = get(url)
    except urllib.error.HTTPError as e:
        print(f"  × {lang}: HTTP {e.code}")
        return None
    except Exception as e:  # noqa: BLE001 - one page must not stop the run
        print(f"  × {lang}: {e}")
        return None
    # A kind that does not exist answers 307 to the hub page.
    if not final.rstrip("/").endswith(url.rstrip("/").rsplit("/", 1)[-1]):
        print(f"  - {lang}: この種類の頁はありません")
        return None
    return cards(body.decode("utf-8", "replace"))


def download(url, dst, force=False):
    if os.path.exists(dst) and not force:
        return "skip"
    _, data = get(url)
    if not data.startswith(b"PK"):
        raise RuntimeError("docx ではありません")
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(dst, "wb") as f:
        f.write(data)
    return "get"


def main(argv=None):
    p = argparse.ArgumentParser(description="Word のテンプレートを英語と日本語の組で集めます")
    p.add_argument("--kinds", default=",".join(KINDS), help="種類をコンマで並べます")
    p.add_argument("--per-kind", type=int, default=3, help="1 種類あたりの数(既定 3。0 で全部)")
    p.add_argument("--limit", type=int, default=0, help="全体の数の上限(0 は無制限)")
    p.add_argument("--force", action="store_true", help="すでにあるファイルも落とし直します")
    p.add_argument("--out", default=ROOT, help="置き場")
    a = p.parse_args(argv)

    kinds = [k.strip() for k in a.kinds.split(",") if k.strip()]
    today = datetime.date.today().isoformat()
    rows, nai, n_get, n_skip, n_err = [], [], 0, 0, 0
    have = set()
    mokuroku = os.path.join(a.out, "目録.tsv")
    if os.path.exists(mokuroku):
        for line in open(mokuroku, encoding="utf-8").read().splitlines()[1:]:
            if line.strip():
                have.add(line.split("\t")[0])

    for kind in kinds:
        if a.limit and len(rows) + len(have) >= a.limit:
            break
        print(f"== {kind}")
        en = one_kind(kind, "en")
        ja = one_kind(kind, "ja") if en is not None else None
        if en is None or ja is None:
            nai.append(kind)
            continue
        ja_map = {g: (t, u) for g, t, u in ja}
        pair = [(g, t, u) for g, t, u in en if g in ja_map]
        print(f"  英語 {len(en)} 個、日本語 {len(ja)} 個、両方にある物 {len(pair)} 個")
        n = 0
        for guid, title_en, url_en in pair:
            if a.per_kind and n >= a.per_kind:
                break
            if a.limit and len(rows) + len(have) >= a.limit:
                break
            title_ja, url_ja = ja_map[guid]
            ok = True
            for lang, url in (("en", url_en), ("ja", url_ja)):
                # en-us is filed by kind (2026-09-21); the other locales
                # sit directly under the locale folder
                dst = (
                    os.path.join(a.out, LOCALE[lang], kind, guid + ".docx")
                    if lang == "en"
                    else os.path.join(a.out, LOCALE[lang], guid + ".docx")
                )
                try:
                    r = download(url, dst, a.force)
                except Exception as e:  # noqa: BLE001 - keep going on one failure
                    print(f"  × {guid} {lang}: {e}")
                    ok = False
                    n_err += 1
                    break
                n_get += r == "get"
                n_skip += r == "skip"
            if not ok:
                continue
            n += 1
            if guid in have:
                continue
            have.add(guid)  # the same template shows up under several kinds
            rows.append([guid, kind, title_en, title_ja, url_en, url_ja, today])
            print(f"  ○ {guid} {title_en} / {title_ja}")

    os.makedirs(a.out, exist_ok=True)
    head = "GUID\t種類\t英語の題\t日本語の題\ten の URL\tja の URL\t落とした日\n"
    new = not os.path.exists(mokuroku)
    with open(mokuroku, "a", encoding="utf-8") as f:
        if new:
            f.write(head)
        for r in rows:
            f.write("\t".join(r) + "\n")
    print(f"\n目録: {mokuroku}")
    print(f"新しく目録に足した組 {len(rows)}、落としたファイル {n_get}、すでにあった物 {n_skip}、失敗 {n_err}")
    if nai:
        print("頁が無かった種類: " + "、".join(nai))
    return 0


if __name__ == "__main__":
    sys.exit(main())
