#!/usr/bin/env python3
"""**利用者が読む文書と画面の文言から、普通の日本語に無い言葉を一覧にします。**

2026-09-18 発注者「否定形をやめませんか。否定だけを外せばいいので意味ないです」。
前の tools/kotoba_check.py は「使わない言葉」の表で見ていました。表にある
言葉を避けるだけで通り、新しい比喩を作れば止められません。この道具は逆で、
**「使う言葉」の側**から見ます。

    python3 tools/kotoba_list.py              # 一覧を出す(合否は出しません)
    python3 tools/kotoba_list.py --top 40     # 多い順に 40 語だけ
    python3 tools/kotoba_list.py --make       # 使う言葉の表を作り直す(要 docx の集め)

「使う言葉」は3つを合わせた物です。

1. Excel と Word の用語(face/src/ribbon_ja.rs のリボンの言葉と、face/src/funcs.rs の
   関数の説明)
2. 役所が公開している docx の本文(~/Documents/officework-cmp/corpus/ と corpus2/。
   500 枚ほど。普通の日本語の見本として使います)
3. tools/kotoba_tsukau_tsuika.txt(発注者が「これは使う」と決めた言葉。1行1語)

1 と 2 から作った表は tools/kotoba_tsukau.txt に置きます(`--make` で作り直す)。
docx の集めが無い機械でも、表があれば回せます。

言葉の切り出しは形態素解析を使わず、形で取ります(mecab などを入れなくても
回るように)。取るのは、漢字の並び(2 字以上)、カタカナの並び(2 字以上)、
漢字+送り仮名+漢字(「組み手」「物差し」の形)、助詞に挟まれた 1 字の漢字
(「の殻を」「の耳に」の形)です。

一覧は多い順で、出てくる場所を 1 つ添えます。**合否は出しません**。読んで、
言い換えるか、tools/kotoba_tsukau_tsuika.txt に足すかを人が決めます。

形で取るので、送り仮名の前の漢字がくっつくことがあります(「実装済み」が
「実装済」で出る)。読み飛ばしてください。辞書を使わない代わりです。
"""

import argparse
import collections
import json
import pathlib
import re
import sys
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TSUKAU = ROOT / "tools" / "kotoba_tsukau.txt"
TSUIKA = ROOT / "tools" / "kotoba_tsukau_tsuika.txt"
CORPUS = [
    pathlib.Path.home() / "Documents" / "officework-cmp" / "corpus",
    pathlib.Path.home() / "Documents" / "officework-cmp" / "corpus2",
]

# 見る文書。利用者が読む物だけ(tools/kotoba_check.py から引き継ぎ)
TARGETS = [
    "README.ja.adoc", "CLAUDE.md",
    "docs/*manual*.adoc", "docs/from-excel*.adoc", "docs/engine*.adoc",
    "docs/mac-signing.ja.adoc", "packaging/README.ja.md", "pysheet/README.md",
    "sample/README.md",
    "docs/ja/*.adoc",
    "docs/ja/commands/*.adoc", "docs/ja/commands/*/*.adoc",
    "ui/i18n/ja.json",
]
SKIP = ("docs/sekkei/", "guide-tsukiawase", ".flatpak-builder/", "docs/en/")

KANJI = "一-龥々〆ヶ"
KATA = "ァ-ヴー"
HIRA = "ぁ-ん"
# 送り仮名として認める仮名。助詞(の・を・が・に・は・で・と・へ・も・ば)は
# 入れません。入れると「名前を付」「違う形」のように語をまたいで取ります
OKURI = "きぎしじちぢっつづてでねびみりれいえけせめゆよらるわ"
WORD_RE = re.compile(
    rf"[{KANJI}]+[{OKURI}]{{1,2}}[{KANJI}]+(?![{HIRA}])"  # 組み手・持ち場・踏み跡
    rf"|[{KANJI}]{{2,}}"                                 # 漢語
    rf"|[{KATA}]{{2,}}"                                  # カタカナ語
)
# 助詞に挟まれた 1 字の漢字(「の殻を」「の耳に」「は口で」)
HITOMOJI_RE = re.compile(
    rf"(?<=[のをがにはでとへも、。（「\s])([{KANJI}])(?=[のをがにはでとへも、。）」\s])"
)


def words_of(text):
    """文から言葉を取ります。1 字の漢字は「1:字」の形で区別します。"""
    out = []
    for m in WORD_RE.finditer(text):
        w = m.group(0)
        # 漢語の中の「々」始まりなどは取らない
        if not w.startswith(("々", "ヶ", "ー")):
            out.append(w)
    for m in HITOMOJI_RE.finditer(text):
        out.append("1:" + m.group(1))
    return out


# ---- 文書を読む(コードや URL は見ない) ---------------------------------------

def strip_adoc(text):
    lines = []
    fence = None
    for line in text.splitlines():
        s = line.strip()
        if fence:
            if s == fence:
                fence = None
            lines.append("")
            continue
        if s in ("----", "....", "```", "++++"):
            fence = s
            lines.append("")
            continue
        if s.startswith((":", "//", "[source", "image::", "include::")):
            lines.append("")
            continue
        line = re.sub(r"`[^`]*`", " ", line)
        line = re.sub(r"https?://\S+", " ", line)
        line = re.sub(r"link:\S+\[", " ", line)
        lines.append(line)
    return lines


def target_files():
    seen = []
    for pat in TARGETS:
        for p in sorted(ROOT.glob(pat)):
            rel = p.relative_to(ROOT).as_posix()
            if p.is_file() and not any(s in rel for s in SKIP):
                seen.append((rel, p))
    return seen


def target_lines(rel, p):
    text = p.read_text(encoding="utf-8")
    if rel.endswith(".json"):
        # 画面の文言。値だけを見ます(キーは英語の記号)
        return [str(v) for v in json.loads(text).values()]
    return strip_adoc(text)


# ---- 使う言葉の表を作る -------------------------------------------------------

def docx_text(path):
    """docx の本文の字。word/document.xml のタグを落とすだけです。"""
    try:
        with zipfile.ZipFile(path) as z:
            names = [n for n in z.namelist() if n.startswith("word/") and n.endswith(".xml")]
            xml = "".join(z.read(n).decode("utf-8", "replace") for n in names)
    except (zipfile.BadZipFile, OSError):
        return ""
    xml = re.sub(r"<w:p[ >]", "\n<w:p ", xml)
    return re.sub(r"<[^>]+>", "", xml)


def rust_strings(path):
    """Rust の表から "…" の中身を全部取ります(用語の見本として)。"""
    text = path.read_text(encoding="utf-8")
    return re.findall(r'"((?:[^"\\]|\\.)*)"', text)


def make_table():
    counter = collections.Counter()
    for rel in ("face/src/ribbon_ja.rs", "face/src/funcs.rs"):
        for s in rust_strings(ROOT / rel):
            counter.update(words_of(s))
    n_docx = 0
    for d in CORPUS:
        if not d.is_dir():
            continue
        for f in sorted(d.glob("*.docx")):
            n_docx += 1
            counter.update(words_of(docx_text(f)))
    if n_docx == 0:
        sys.exit("docx の集めが見つかりません(~/Documents/officework-cmp/corpus/)。--make はそこで回してください")
    with TSUKAU.open("w", encoding="utf-8") as f:
        f.write("# 使う言葉の表。tools/kotoba_list.py --make が作ります(手で直さない)。\n")
        f.write(f"# Excel と Word の用語(ribbon_ja / funcs)と、役所の docx {n_docx} 枚の本文から。\n")
        for w, c in sorted(counter.items(), key=lambda x: (-x[1], x[0])):
            f.write(f"{w}\t{c}\n")
    print(f"使う言葉の表を作りました: {len(counter)} 語(docx {n_docx} 枚)→ {TSUKAU.relative_to(ROOT)}")


def load_table():
    words = set()
    for p in (TSUKAU, TSUIKA):
        if not p.exists():
            continue
        for line in p.read_text(encoding="utf-8").splitlines():
            s = line.strip()
            if s and not s.startswith("#"):
                words.add(s.split("\t")[0])
    return words


# ---- 一覧 ---------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--make", action="store_true", help="使う言葉の表を作り直す")
    ap.add_argument("--top", type=int, default=0, help="多い順に何語まで出すか(0 で全部)")
    ap.add_argument("--min", type=int, default=1, help="この回数以上の言葉だけ")
    a = ap.parse_args()
    if a.make:
        make_table()
        return 0
    if not TSUKAU.exists():
        sys.exit(f"{TSUKAU.relative_to(ROOT)} がありません。docx の集めのある機械で --make を回してください")
    known = load_table()
    count = collections.Counter()
    where = {}
    n_files = 0
    for rel, p in target_files():
        n_files += 1
        for n, line in enumerate(target_lines(rel, p), 1):
            for w in words_of(line):
                if w in known:
                    continue
                count[w] += 1
                where.setdefault(w, (rel, n, line.strip()[:50]))
    rows = [(w, c) for w, c in count.items() if c >= a.min]
    rows.sort(key=lambda x: (-x[1], x[0]))
    if a.top:
        rows = rows[: a.top]
    print(f"文書 {n_files} 枚。使う言葉の表({len(known)} 語)に無い言葉が {len(count)} 語"
          f"(出す物 {len(rows)} 語)。合否ではありません。読んで、言い換えるか"
          f" tools/kotoba_tsukau_tsuika.txt に足すかを決めてください。\n")
    for w, c in rows:
        rel, n, line = where[w]
        shown = w[2:] + "(1字)" if w.startswith("1:") else w
        print(f"{c:5d}  {shown:12}  {rel}:{n}  {line}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
