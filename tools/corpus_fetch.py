#!/usr/bin/env python3
"""**公開されている docx を、一覧のページから集める道具。**

官公庁・自治体の「様式集」「申請書のダウンロード」のページには、Word で
作った docx がまとまって置いてあります。ページの中の docx へのリンクを
抜いて、`~/docx-corpus/<ホスト名>/` に落とします(2026-09-09 発注者)。

    python3 tools/corpus_fetch.py <一覧のページの URL> [URL …]
    python3 tools/corpus_fetch.py --out DIR URL …

落とした物の目録は `<置き場>/目録.tsv` に足します(取った日、出所の URL、
大きさ、sha256 の頭、置いた径路)。現物は repo に置きません
(docs/corpus-docx.ja.adoc)。同じ名前の物があれば飛ばします。
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


def docx_links(page_url, html):
    out = []
    for m in re.finditer(r'href\s*=\s*["\']([^"\']+\.docx)(?:[?#][^"\']*)?["\']', html, re.I):
        out.append(urllib.parse.urljoin(page_url, m.group(1)))
    seen, uniq = set(), []
    for u in out:
        if u not in seen:
            seen.add(u)
            uniq.append(u)
    return uniq


def fetch_all(pages, out_dir):
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
        links = docx_links(page, html)
        print(f"== {page}: docx {len(links)} 個")
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
    out = os.path.expanduser("~/docx-corpus")
    if "--out" in args:
        i = args.index("--out")
        out = args[i + 1]
        del args[i:i + 2]
    if not args:
        raise SystemExit(__doc__)
    fetch_all(args, out)
