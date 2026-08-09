#!/usr/bin/env python3
"""genoffice の docx エンジンと officework.doc に同じ docx を通し、答えを比べる。

xlsx でやったこと(tools/pyoffice_diff.py)の docx 版。**自分で書いた文書だけで
往復を確かめると、環境依存と実物特有の形を必ず見落とす** — 2026-08-09 に
`sheetView` を Empty の枝でしか読んでいなかったのを踏んだのがそれ。
向こう(TypeScript、packages/docx-engine)の答えを正解表として、こちらの穴を数で出す。

使い方:

    python3 tools/pydoc_diff.py                        # sample/ を全部
    python3 tools/pydoc_diff.py 報告書.docx …          # ファイルを指定
    python3 tools/pydoc_diff.py --corpus DIR           # DIR/*.docx を足す
    python3 tools/pydoc_diff.py --json out.json        # 差の一覧を書き出す
    python3 tools/pydoc_diff.py --show 20              # 1件ごとに出す**例**の上限(既定 5)
    python3 tools/pydoc_diff.py --roundtrip            # 開いて保存した物を向こうに読ませる

`--show` は**出す例を絞るだけで、差の数は減らしません**。まとめの行には
絞る前の総数が必ず出ます。絞った出力を総数と読んだ事故が 2026-08-10 に
xlsx 側で起きた(7枚と報告した所が実は 58,487 件)ので、そう作ってあります。

向こうの木の場所は環境変数 GENOFFICE で変えられる(既定 ~/dev/genoffice)。
向こうは TypeScript なので tsx が要る。**向こうの木には何も置かない** —
入口を動的 import で借りるだけ(設計の「genoffice には何もしない」に従う)。
tsx が入っていなければ、入れ方を言って終わる。

うちのエンジンを動かす python は OFFICEWORK_PYTHON で選ぶ(既定はこの python)。
**repo の直下で素の python を使うと、.venv の officework.pth がソースの方
(エンジンの入っていない officework/)を先に掴む** — wheel か、cargo build した
.so を置いた木を指すこと。

## --roundtrip — **売り文句そのものを測る**

`Doc.open` して `save` しただけの物を作り、**原本と保存後を「向こうの読み手」に
読ませて比べる。** 自分の読み手で往復を確かめると、読めていない所は行きも帰りも
同じように読めないので差が出ない(自分の物差しで自分を測る形)。
審判を向こうに任せると、そこが出る。zip の部品が減っていないかも見る。

## 何を比べていて、何を比べていないか

比べる: **本文の段落の字**(順番どおり)・**表のセルの字**(行×列)・
ヘッダーとフッターの字。どちらも「読めた字」だけを突き合わせる。

比べない(**比べられない**): 書式・様式・図形・変更履歴・しおり・脚注。
向こうの `parseDocx` は返すが、`officework.doc` の窓が返さない
(段落と表に絞ってあるのは、細かすぎる模型を Python に出さないため)。
**ただし保存では原本から持ち越される** — 読めていないことと落とすことは別。
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys

GENOFFICE = pathlib.Path(os.environ.get("GENOFFICE", os.path.expanduser("~/dev/genoffice")))
ENGINE = GENOFFICE / "packages/docx-engine/src/index.ts"
HERE = pathlib.Path(__file__).resolve().parent
DUMP = HERE / "docx_dump.mts"

# うちのエンジンは**別プロセスで動かす**(pyoffice_diff.py と同じ理由 —
# repo の直下だと officework.pth がソースの方を先に掴む)
OUR_PY = os.environ.get("OFFICEWORK_PYTHON", sys.executable)

_OUR_SCRIPT = r"""
import json, pathlib, sys
from officework import doc

d = doc.Doc.open(str(pathlib.Path(sys.argv[1]).resolve()))
print(json.dumps({
    "paragraphs": [p.text for p in d.paragraphs],
    "tables": [t.values() for t in d.tables],
    "header": d.header.split("\n") if d.header else [],
    "footer": d.footer.split("\n") if d.footer else [],
    "unsupported": d.unsupported,
}, ensure_ascii=False))
"""


def find_tsx():
    """tsx の在り処。向こうの木のどこに入っていても拾う。"""
    env = os.environ.get("TSX")
    if env:
        return pathlib.Path(env)
    for p in (
        GENOFFICE / "node_modules/.bin/tsx",
        GENOFFICE / "packages/docx-engine/node_modules/.bin/tsx",
    ):
        if p.exists():
            return p
    return None


def theirs_view(path, tsx):
    """向こう(TypeScript)の答え。"""
    r = subprocess.run(
        [str(tsx), str(DUMP), str(pathlib.Path(path).resolve()), str(ENGINE)],
        capture_output=True,
        text=True,
        encoding="utf-8",
        cwd=str(GENOFFICE),
    )
    if r.returncode != 0:
        return {"error": (r.stderr or r.stdout).strip()[:400]}
    return json.loads(r.stdout)


def our_view(path):
    """うちのエンジンの答えを、同じ形に揃える(別プロセス)。"""
    r = subprocess.run(
        [OUR_PY, "-c", _OUR_SCRIPT, str(path)],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if r.returncode != 0:
        return {"error": (r.stderr or r.stdout).strip()[:400]}
    return json.loads(r.stdout)


_SAVE_SCRIPT = r"""
import pathlib, sys
from officework import doc
d = doc.Doc.open(str(pathlib.Path(sys.argv[1]).resolve()))
d.save(sys.argv[2])
"""


def our_roundtrip(path, dst):
    """開いて保存しただけの物を作る(中身は触らない)。"""
    r = subprocess.run(
        [OUR_PY, "-c", _SAVE_SCRIPT, str(path), str(dst)],
        capture_output=True, text=True, encoding="utf-8",
    )
    return None if r.returncode == 0 else (r.stderr or r.stdout).strip()[:400]


def diff_roundtrip(path, tsx, show):
    """原本と「開いて保存した物」を、**向こうの読み手に**読ませて比べる。"""
    import tempfile, zipfile

    diffs, theirs_bug, total = [], [], 0
    with tempfile.TemporaryDirectory() as t:
        out = pathlib.Path(t) / "roundtrip.docx"
        err = our_roundtrip(path, out)
        if err:
            return {}, {}, [f"うちが開いて保存できない: {err.splitlines()[-1]}"], [], 1
        before, after = theirs_view(path, tsx), theirs_view(out, tsx)
        if "error" in before:
            return before, after, [], [f"向こうが原本を読めない: {before['error'].splitlines()[0]}"], 0
        if "error" in after:
            return before, after, [f"保存した物を向こうが読めない: {after['error'].splitlines()[0]}"], [], 1
        for lines, k in (
            diff_seq("段落", before["paragraphs"], after["paragraphs"], show),
            diff_tables(before["tables"], after["tables"], show),
            diff_seq("ヘッダー", before["header"], after["header"], show),
            diff_seq("フッター", before["footer"], after["footer"], show),
        ):
            diffs += lines
            total += k
        # **部品が減っていないか。** 原本を正として書き戻すのが売り文句なので、
        # ここが減っていたら主張ごと崩れる
        with zipfile.ZipFile(path) as z:
            a = set(z.namelist())
        with zipfile.ZipFile(out) as z:
            b = set(z.namelist())
        if a - b:
            diffs.append(f"[部品] 保存で消えた: {sorted(a - b)}")
            total += len(a - b)
    return before, after, diffs, theirs_bug, total


def norm(s):
    """比べる前の均し。**中身は削らない** — 見えない差だけを落とす。

    - 改行と復帰は空白1つに(段落の区切りは配列で持っているので、字の中の
      改行はどちらが持つかの流儀の違いにすぎない)
    - ノーブレークスペースは普通の空白に(Word がよく入れる)
    - 前後の空白は落とす
    """
    return (
        s.replace("\r", " ")
        .replace("\n", " ")
        .replace(" ", " ")
        .strip()
    )


def diff_seq(kind, a, b, show):
    """字の並びを突き合わせる。a=向こう、b=うち。→ (出す行, 差の総数)

    **`show` は出す例を絞るだけで、数えるのは最後まで数える。**
    絞った出力を総数と読んで事故が起きたので(2026-08-10、xlsx 側で
    7枚と報告した所が実は 58,487 件だった)、真の数は必ず持ち帰る。
    """
    out, n = [], 0
    # 空の段落は数え方の流儀が割れる(向こうは画像だけの段落も1つと数える)。
    # **落とさずに、まず素で比べる** — 揃わないときだけ中を出す
    if len(a) != len(b):
        out.append(f"[{kind}] 数が違う: 向こう {len(a)} / うち {len(b)}")
        n += 1
    for i, (x, y) in enumerate(zip(a, b)):
        if norm(x) != norm(y):
            n += 1
            if len(out) < show:
                out.append(f"[{kind}] {i}: 向こう {norm(x)!r} / うち {norm(y)!r}")
    if n > len(out):
        out.append(f"[{kind}] ほか {n - len(out)} 件(--show で絞ってある)")
    return out, n


def diff_tables(a, b, show):
    """表を突き合わせる。→ (出す行, 差の総数)。数えるのは最後まで([`diff_seq`])。"""
    out, n = [], 0

    def note(line):
        nonlocal n
        n += 1
        if len(out) < show:
            out.append(line)

    if len(a) != len(b):
        note(f"[表] 数が違う: 向こう {len(a)} / うち {len(b)}")
    for ti, (ta, tb) in enumerate(zip(a, b)):
        if len(ta) != len(tb):
            note(f"[表{ti}] 行数が違う: 向こう {len(ta)} / うち {len(tb)}")
        for ri, (ra, rb) in enumerate(zip(ta, tb)):
            if len(ra) != len(rb):
                note(f"[表{ti}] {ri}行目の列数が違う: 向こう {len(ra)} / うち {len(rb)}")
            for ci, (ca, cb) in enumerate(zip(ra, rb)):
                if norm(ca) != norm(cb):
                    note(f"[表{ti}] {ri}行{ci}列: 向こう {norm(ca)!r} / うち {norm(cb)!r}")
    if n > len(out):
        out.append(f"[表] ほか {n - len(out)} 件(--show で絞ってある)")
    return out, n


def compare(path, tsx, show):
    theirs = theirs_view(path, tsx)
    ours = our_view(path)
    diffs, theirs_bug, total = [], [], 0

    if "error" in theirs:
        # **向こうが読めない物は、うちの宿題ではない。** 数に混ぜない
        theirs_bug.append(f"向こうが読めない: {theirs['error'].splitlines()[0]}")
        return theirs, ours, diffs, theirs_bug, total
    if "error" in ours:
        diffs.append(f"うちが読めない: {ours['error'].splitlines()[0]}")
        return theirs, ours, diffs, theirs_bug, 1

    for lines, k in (
        diff_seq("段落", theirs["paragraphs"], ours["paragraphs"], show),
        diff_tables(theirs["tables"], ours["tables"], show),
        diff_seq("ヘッダー", theirs["header"], ours["header"], show),
        diff_seq("フッター", theirs["footer"], ours["footer"], show),
    ):
        diffs += lines
        total += k
    # **読めなかった物は差ではないが、黙らせない。** 帳簿に出ているのが正しい姿
    if ours.get("unsupported"):
        theirs_bug.append(f"うちが未対応と申告した物: {ours['unsupported']}")
    return theirs, ours, diffs, theirs_bug, total


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="*")
    ap.add_argument("--corpus", action="append", default=[], help="この DIR の *.docx も足す")
    ap.add_argument("--json", help="差の一覧の書き出し先")
    ap.add_argument("--show", type=int, default=5, help="1件ごとに出す差の上限")
    ap.add_argument(
        "--roundtrip", action="store_true",
        help="開いて保存した物を向こうに読ませ、原本と比べる(売り文句そのものの検査)",
    )
    a = ap.parse_args()

    if not ENGINE.exists():
        sys.exit(f"向こうの docx エンジンがありません: {ENGINE}\n  GENOFFICE= で木を指してください")
    tsx = find_tsx()
    if tsx is None:
        sys.exit(
            f"tsx がありません(向こうは TypeScript のまま配られています)\n"
            f"  cd {GENOFFICE} && npm install\n"
            f"  か、TSX=/path/to/tsx で指してください"
        )

    files = [pathlib.Path(f) for f in a.files]
    for d in a.corpus:
        files += sorted(pathlib.Path(d).glob("*.docx"))
    if not files:
        for d in ("sample", "sample/writer", "templates"):
            files += sorted(pathlib.Path(d).glob("*.docx"))
    files = [f for f in files if not f.name.startswith("~$")]
    if not files:
        sys.exit("比べる docx がありません")

    report = []
    for f in files:
        if a.roundtrip:
            theirs, ours, diffs, theirs_bug, k = diff_roundtrip(f, tsx, a.show)
        else:
            theirs, ours, diffs, theirs_bug, k = compare(f, tsx, a.show)
        report.append(
            {"file": str(f), "diffs": diffs, "diff_count": k, "theirs_bug": theirs_bug,
             "theirs": theirs, "ours": ours}
        )
        # **△ = 差はあるが、うちの宿題ではない物だけ。**
        # ○ と混ぜると宿題が水増しされ、× と混ぜると直せない物を追いかける
        mark = "×" if diffs else ("△" if theirs_bug else "○")
        print(f"{mark} {f}")
        for d in diffs:
            print(f"    {d}")
        for d in theirs_bug:
            print(f"    ~ {d}")

    n = sum(1 for r in report if r["diffs"])
    m = sum(1 for r in report if not r["diffs"] and r["theirs_bug"])
    side = "theirs" if a.roundtrip else "ours"
    paras = sum(len(r[side].get("paragraphs") or []) for r in report)
    cells = sum(
        len(row) for r in report for t in (r[side].get("tables") or []) for row in t
    )
    # **例示の上限で数を誤読させない。** `--show` は1件あたりの**例**を絞るだけで、
    # 差の総数は減らない。絞った出力を総数と読んだ事故が 2026-08-10 に xlsx 側で
    # 起きている(7枚と報告した所が実は 58,487 件)。**総数を必ず出す。**
    # pyoffice_diff.py は出力の字を数え直しているが、こちらは走査中に真の数を
    # 持っているので、そのまま持ち帰っている
    total = sum(r["diff_count"] for r in report)
    print(
        f"\n{len(report)} 枚中 {n} 枚で差が出ました"
        f"(比べた段落 {paras} ・セル {cells} / 差の総数 {total} 件)"
    )
    if total and a.show < 20:
        print(f"  ※ --show {a.show} は**例を絞っているだけ**。総数は上の {total} 件")
    if m:
        print(f"  ほかに {m} 枚は △ — うちの宿題ではない差だけ")
    # **緑を大きく見せない。** 何を見ていないかを毎回言う
    if a.roundtrip:
        print(
            "  見たのは、向こうの読み手が読める範囲(段落・表・ヘッダー・フッター)と "
            "zip の部品の数。書式そのものの一致は見ていない"
        )
    else:
        print(
            "  比べていない: 書式・様式・図形・変更履歴・しおり・脚注 "
            "(officework.doc の窓が返さない。保存では原本から持ち越される)"
        )
    if a.json:
        pathlib.Path(a.json).write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(f"書き出し: {a.json}")
    return 1 if n else 0


if __name__ == "__main__":
    sys.exit(main())
