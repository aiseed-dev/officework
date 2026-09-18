#!/usr/bin/env python3
"""**A tool that collects published docx and xlsx files from an index page.**

Pages such as the form collections and application-form downloads of national and local
government sites keep documents made with Word and Excel in one place. This tool pulls
the links out of the page and downloads the files into `~/docx-corpus/<host name>/`
(decided by the owner on 2026-09-09).

    python3 tools/corpus_fetch.py <URL of the index page> [URL …]
    python3 tools/corpus_fetch.py --out DIR URL …
    python3 tools/corpus_fetch.py --ext xlsx --out ~/xlsx-corpus URL …

`--ext` is the extension to download (docx by default). Passing `xlsx` picks up both
`.xlsx` and `.xlsm`. A list of what was downloaded is appended to `<directory>/目録.tsv`
(the date, the source URL, the size, the start of the sha256 and the path it was saved
to). The files themselves are not kept in the repo (docs/corpus-docx.ja.adoc,
docs/corpus.ja.adoc). A file whose name is already there is skipped.
"""
import datetime
import hashlib
import os
import re
import sys
import urllib.parse
import urllib.request

UA = "Mozilla/5.0 (officework corpus fetch)"
MAX_BYTES = 20 * 1024 * 1024


def get(url, timeout=60):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()


EXTS = {"docx": ("docx",), "xlsx": ("xlsx", "xlsm"), "pptx": ("pptx",)}


def doc_links(page_url, html, ext="docx"):
    """ページの中の、その拡張子へのリンクを URL の一覧にします。"""
    pat = "|".join(EXTS.get(ext, (ext,)))
    out = []
    for m in re.finditer(rf'href\s*=\s*["\']([^"\']+\.(?:{pat}))(?:[?#][^"\']*)?["\']', html, re.I):
        out.append(urllib.parse.urljoin(page_url, m.group(1)))
    seen, uniq = set(), []
    for u in out:
        if u not in seen:
            seen.add(u)
            uniq.append(u)
    return uniq


def fetch_all(pages, out_dir, ext="docx"):
    os.makedirs(out_dir, exist_ok=True)
    log = os.path.join(out_dir, "目録.tsv")
    today = datetime.date.today().isoformat()
    n_ok = 0
    for page in pages:
        try:
            html = get(page).decode("utf-8", "replace")
        except Exception as e:
            print(f"× {page}: {e}")
            continue
        links = doc_links(page, html, ext)
        print(f"== {page}: {ext} {len(links)} 個")
        host = urllib.parse.urlparse(page).hostname or "unknown"
        d = os.path.join(out_dir, host)
        os.makedirs(d, exist_ok=True)
        for u in links:
            name = urllib.parse.unquote(os.path.basename(urllib.parse.urlparse(u).path))
            dst = os.path.join(d, name)
            if os.path.exists(dst):
                continue
            try:
                data = get(u)
            except Exception as e:
                print(f"  × {u}: {e}")
                continue
            if len(data) > MAX_BYTES or not data.startswith(b"PK"):
                print(f"  - 飛ばす({len(data)} バイト、zip でない?): {u}")
                continue
            with open(dst, "wb") as f:
                f.write(data)
            sha = hashlib.sha256(data).hexdigest()[:16]
            with open(log, "a", encoding="utf-8") as f:
                f.write(f"{today}\t{u}\t{len(data)}\t{sha}\t{os.path.relpath(dst, out_dir)}\n")
            n_ok += 1
            print(f"  + {name} ({len(data)})")
    print(f"落とした数: {n_ok}")


if __name__ == "__main__":
    args = sys.argv[1:]
    ext = "docx"
    if "--ext" in args:
        i = args.index("--ext")
        ext = args[i + 1].lstrip(".").lower()
        del args[i:i + 2]
    out = os.path.expanduser(f"~/{ext}-corpus")
    if "--out" in args:
        i = args.index("--out")
        out = os.path.expanduser(args[i + 1])
        del args[i:i + 2]
    if not args:
        raise SystemExit(__doc__)
    fetch_all(args, out, ext)
