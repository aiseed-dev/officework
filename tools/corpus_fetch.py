#!/usr/bin/env python3
"""**公開されている docx や xlsx を、一覧のページから集める道具。**

官公庁・自治体の「様式集」「申請書のダウンロード」のページには、Word や
Excel で作った書類がまとまって置いてあります。ページの中のリンクを抜いて、
`~/docx-corpus/<ホスト名>/` に落とします(2026-09-09 発注者)。

    python3 tools/corpus_fetch.py <一覧のページの URL> [URL …]
    python3 tools/corpus_fetch.py --out DIR URL …
    python3 tools/corpus_fetch.py --ext xlsx --out ~/xlsx-corpus URL …

`--ext` は落とす拡張子です(既定は docx)。`xlsx` を渡すと `.xlsx` と
`.xlsm` を拾います。落とした物の目録は `<置き場>/目録.tsv` に足します
(取った日、出所の URL、大きさ、sha256 の頭、置いた径路)。現物は repo に
置きません(docs/corpus-docx.ja.adoc、docs/corpus.ja.adoc)。同じ名前の
物があれば飛ばします。
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
