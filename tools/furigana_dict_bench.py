"""辞書だけで、人手ルビがどこまで当たるかを**文の中で**測る。

**外に2つ要ります**(どちらもこの repo には置きません)。

* `mecab` と `ipadic-utf8` — `sudo apt install mecab mecab-ipadic-utf8`
* `pybunko` — 青空文庫の取得と青空注記の解析。`~/dev/bunko`(aiseed-dev/bunko)。
  入れずに `sys.path` で届きます

コーパスの現物も置きません(docs/corpus.ja.adoc の決め)。`pybunko.Library` が
カタログを引いて取り、`aozora_cache/` に貯めるので2度目からは網に出ません。

前の測り方(語を単独で引く)は不公平でした。青空のルビは漢字の部分だけに
付くのに(呟《つぶや》く)、辞書は送り仮名込みの語(呟く/つぶやく)で
持っているためです。**文をそのまま解析して、その位置の語の読みと比べます。**

比べ方:
  * 語の切れ目が親字と同じ      → 読みが一致するか
  * 語が親字で始まる(送り仮名) → 読みがルビで始まるか
  * 親字が複数の語にまたがる    → つないだ読みと比べる

使い方:  python3 bench_ctx.py [作品ID...]
"""
import pathlib
import subprocess
import sys
from collections import Counter

sys.path.insert(0, "/home/dev/dev/bunko")
from pybunko import Library  # noqa: E402

DIC = "/var/lib/mecab/dic/ipadic-utf8"
CACHE = "/home/dev/dev/pykobo/aozora_cache"


def kata_to_hira(s: str) -> str:
    return "".join(
        chr(ord(c) - 0x60) if 0x30A1 <= ord(c) <= 0x30F6 else c for c in s
    )


def analyze(text: str) -> list[tuple[int, str, str]]:
    """文を解析して (開始位置, 表層, 読み) を返す。読みはひらがな。"""
    r = subprocess.run(
        ["mecab", "-d", DIC], input=text + "\n",
        capture_output=True, text=True, timeout=120,
    )
    out, at = [], 0
    for line in r.stdout.splitlines():
        if line == "EOS":
            break
        f = line.split("\t")
        if len(f) < 2:
            continue
        surf = f[0]
        a = f[1].split(",")
        yomi = kata_to_hira(a[7]) if len(a) > 7 and a[7] != "*" else ""
        # 解析は入力の字をそのまま並べるので、順に足せば位置が出る
        at = text.find(surf, at)
        if at < 0:
            break
        out.append((at, surf, yomi))
        at += len(surf)
    return out


def judge(toks, start, base, ruby) -> tuple[bool, str]:
    """その位置の語の読みが、人手ルビと合うか。(合ったか, 辞書が出した読み)"""
    same = [t for t in toks if t[0] == start]
    if not same:
        return False, "(位置が合わない)"
    at, surf, yomi = same[0]
    if surf == base:
        return (yomi == ruby), yomi
    if surf.startswith(base):
        # 送り仮名つきの語。読みがルビで始まれば当たり
        return yomi.startswith(ruby), yomi
    # 親字が複数の語にまたがる
    acc_s, acc_y, i = "", "", toks.index(same[0])
    while i < len(toks) and len(acc_s) < len(base):
        acc_s += toks[i][1]
        acc_y += toks[i][2]
        i += 1
    if acc_s == base:
        return (acc_y == ruby), acc_y
    if acc_s.startswith(base):
        return acc_y.startswith(ruby), acc_y
    return False, acc_y or "(切れ方が違う)"


def main() -> int:
    lib = Library(cache_dir=CACHE)
    ids = sys.argv[1:]
    if ids:
        want = {i.lstrip("0") for i in ids}
        works = [w for w in lib.works if w.work_id.lstrip("0") in want]
    else:
        have = {p.name.split("_")[0] for p in pathlib.Path(CACHE).glob("*.zip")}
        have.discard("catalog")
        norm = {h.lstrip("0") for h in have}
        works = [w for w in lib.works if w.work_id.lstrip("0") in norm]
    if not works:
        print("測る作品がありません")
        return 1

    hit = miss = 0
    misses: Counter = Counter()
    per = []
    for w in works:
        doc = w.document()
        n = 0
        wh = wm = 0
        for p in doc.paragraphs:
            # 段落の中でのルビの位置を、切れ端の長さを足して出す
            plain, spans = "", []
            for text, ruby in p.segments:
                if ruby:
                    spans.append((len(plain), text, kata_to_hira(ruby)))
                plain += text
            if not spans:
                continue
            toks = analyze(plain)
            for start, base, ruby in spans:
                ok, got = judge(toks, start, base, ruby)
                n += 1
                if ok:
                    hit += 1
                    wh += 1
                else:
                    miss += 1
                    wm += 1
                    misses[(base, ruby, got)] += 1
        per.append((w.title, w.author, wh, wm))

    print()
    print(f"{"作品":22} {"ルビ":>5} {"当たり":>7} {"率":>7}")
    print("-" * 46)
    for t, a, wh, wm in sorted(per, key=lambda x: -(x[2] + x[3])):
        tt = wh + wm
        print(f"{t[:20]:22} {tt:5} {wh:7} {wh / tt:7.1%}" if tt else f"{t[:20]:22} {0:5}")
    tot = hit + miss
    print()
    print(f"文の中で当たった  {hit:6} / {tot}  = {hit / tot:6.1%}")
    print(f"外した            {miss:6} / {tot}  = {miss / tot:6.1%}")
    print()
    print("--- 外した物(多い順に25)---")
    for (b, ans, got), k in misses.most_common(25):
        print(f"  {k:3}回  {b}《{ans}》  辞書は {got!r}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
