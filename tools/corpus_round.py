#!/usr/bin/env python3
"""**集めた docx を、officework と Word の両方で PDF にして比べる1周。**

`tools/corpus_fetch.py` で落とした docx(既定 `~/docx-corpus/`)を1枚ずつ、
officework(`Doc.open().to_pdf()`)と Word(`tools/ms_pdf.py`)で PDF にし、
`tools/ms_compare.py` で比べます。結果は出力先(既定
`~/Documents/officework-cmp/corpus/`)に `<名前>.ow.pdf` / `.ms.pdf` /
`.diff.txt` と、まとめの `まとめ.tsv`(違いの多い順)を置きます。

    .venv/bin/python tools/corpus_round.py [--corpus DIR] [--out DIR] [--limit N] [--only 名前]

Word の PDF は1度作れば取っておきます(`--redo-ms` で作り直し)。
officework の PDF は毎回作り直します(直した後に回すのが目的なので)。
Word が5分を超えて返らない docx は飛ばして記録します(発注者の Word は落としません)。
Word に開かせるのは出力先に置いた写しです(別の場所だと Word がアクセスの許可を
画面で聞いて返らない)。
"""
import argparse
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tools"))
import ms_compare  # noqa: E402

PY = os.path.join(ROOT, ".venv", "bin", "python")


def ours(src, out):
    code = ("import sys\nfrom officework import doc\n"
            "doc.Doc.open(sys.argv[1]).to_pdf(sys.argv[2])\n")
    env = dict(os.environ, DYLD_FALLBACK_LIBRARY_PATH=os.path.expanduser("~/miniforge3/lib"))
    r = subprocess.run([PY, "-c", code, src, out], capture_output=True, text=True, timeout=180, env=env)
    if r.returncode != 0:
        return r.stderr.strip().splitlines()[-1] if r.stderr.strip() else f"exit {r.returncode}"
    return None


def word(src, out):
    try:
        r = subprocess.run([sys.executable, os.path.join(ROOT, "tools", "ms_pdf.py"), src, out],
                           capture_output=True, text=True, timeout=300)
    except subprocess.TimeoutExpired:
        return "Word が 5 分返さない"
    if r.returncode != 0:
        return (r.stderr.strip() or r.stdout.strip()).splitlines()[-1]
    return None


def strip_comments(path):
    """写しからコメントを外します(2026-09-09)。コメントのある docx は、Word の PDF に
    コメントの欄が右に付いて紙面全体が 0.72 倍に縮み、比べられません(厚労省の
    研究費様式 4 枚)。comments.xml と本文の印(commentRangeStart / End /
    commentReference)を消します。うちも同じ写しを読むので、両方とも本文だけになります"""
    import re
    import zipfile
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        if "word/comments.xml" not in names:
            return False
        items = [(n, z.read(n)) for n in names]
    tmp = path + ".tmp"
    with zipfile.ZipFile(tmp, "w", zipfile.ZIP_DEFLATED) as out:
        for n, data in items:
            if n in ("word/comments.xml", "word/commentsExtended.xml", "word/commentsIds.xml", "word/commentsExtensible.xml"):
                continue
            if n == "word/document.xml":
                t = data.decode("utf-8")
                t = re.sub(r"<w:commentRange(?:Start|End) [^>]*/>", "", t)
                t = re.sub(r"<w:r>(?:<w:rPr>(?:(?!</w:rPr>).)*</w:rPr>)?<w:commentReference [^>]*/></w:r>", "", t)
                t = re.sub(r"<w:commentReference [^>]*/>", "", t)
                data = t.encode("utf-8")
            if n == "word/_rels/document.xml.rels":
                t = data.decode("utf-8")
                t = re.sub(r'<Relationship [^>]*Target="comments[^"]*\.xml"[^>]*/>', "", t)
                data = t.encode("utf-8")
            if n == "[Content_Types].xml":
                t = data.decode("utf-8")
                t = re.sub(r'<Override [^>]*PartName="/word/comments[^"]*\.xml"[^>]*/>', "", t)
                data = t.encode("utf-8")
            out.writestr(n, data)
    os.replace(tmp, path)
    return True


def copy_in(src, dst):
    import shutil
    if not os.path.exists(dst) or os.path.getmtime(dst) < os.path.getmtime(src):
        shutil.copyfile(src, dst)
        if strip_comments(dst):
            # コメントを外した写しは Word の PDF も作り直す
            ms = dst[:-5] + ".ms.pdf"
            if os.path.exists(ms):
                os.remove(ms)
    return dst


def pages(pdf):
    import pdfplumber
    with pdfplumber.open(pdf) as p:
        return len(p.pages)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default=os.path.expanduser("~/docx-corpus"))
    ap.add_argument("--out", default=os.path.expanduser("~/Documents/officework-cmp/corpus"))
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--only", default=None)
    ap.add_argument("--redo-ms", action="store_true")
    ap.add_argument("--ms-only", action="store_true", help="Word の PDF だけ作る(うちの側は後で)")
    a = ap.parse_args()
    files = []
    for d, _, fs in os.walk(a.corpus):
        for f in sorted(fs):
            if f.lower().endswith(".docx") and not f.startswith("~$"):
                files.append(os.path.join(d, f))
    # 比べる対象から外す物(Word の PDF が普通でない)。理由を添えて増やす
    hazusu = {
        # Word の PDF で 1 行目の字が 2 つの文に重なって出る。Word 側の出力が普通でない
        "jsite.mhlw.go.jp/002517302.docx",
    }
    files = [f for f in files if os.path.relpath(f, a.corpus) not in hazusu]
    if a.only:
        files = [f for f in files if a.only in f]
    if a.limit:
        files = files[:a.limit]
    os.makedirs(a.out, exist_ok=True)
    rows = []
    for src in files:
        rel = os.path.relpath(src, a.corpus)
        stem = os.path.join(a.out, rel[:-5].replace(os.sep, "__"))
        # Word には ~/Documents の下の写しを開かせます。別の場所のファイルだと
        # Word が「アクセスを許可するか」を画面で聞き、AppleScript が返らない
        src = copy_in(src, stem + ".docx")
        ow, ms, diff = stem + ".ow.pdf", stem + ".ms.pdf", stem + ".diff.txt"
        t0 = time.time()
        if a.ms_only:
            if a.redo_ms or not os.path.exists(ms):
                e = word(src, ms)
                print(f"{'×' if e else '+'} {rel}: {e or 'Word の PDF'} {time.time() - t0:.0f}s", flush=True)
                if e and "閉じません" in e:
                    break
            continue
        e = ours(src, ow)
        if e:
            rows.append((rel, -1, 0, 0, f"うちが PDF にできない: {e}"))
            print(f"× {rel}: うち: {e}", flush=True)
            continue
        if a.redo_ms or not os.path.exists(ms):
            e = word(src, ms)
            if e and "閉じません" in e:
                # Word が文書を閉じない状態。続けると窓が溜まるので、ここで止める
                print(f"× {rel}: {e}", flush=True)
                break
            if e:
                rows.append((rel, -1, pages(ow), 0, f"Word が PDF にできない: {e}"))
                print(f"× {rel}: Word: {e}", flush=True)
                continue
        d = ms_compare.compare(ow, ms)
        with open(diff, "w", encoding="utf-8") as f:
            f.write("\n".join(d) + f"\n違い {len(d)} 件\n")
        po, pm = pages(ow), pages(ms)
        rows.append((rel, len(d), po, pm, d[0] if d else ""))
        print(f"== {rel}: 違い {len(d)} 件 (頁 {po}/{pm}) {time.time() - t0:.0f}s", flush=True)
    if a.ms_only:
        return
    rows.sort(key=lambda r: (-r[1], r[0]))
    with open(os.path.join(a.out, "まとめ.tsv"), "w", encoding="utf-8") as f:
        f.write("ファイル\t違い\tうちの頁\tWord の頁\t最初の違い\n")
        for r in rows:
            f.write("\t".join(str(x) for x in r) + "\n")
    ok = sum(1 for r in rows if r[1] == 0)
    print(f"一致 {ok} / {len(rows)}")


if __name__ == "__main__":
    main()
