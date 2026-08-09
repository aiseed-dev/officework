#!/usr/bin/env python3
"""genoffice のサイドカーと officework のエンジンに同じ xlsx を通し、答えを比べる。

**実装から入らないための道具**(SEKKEI docs/sekkei/pyoffice.ja.md)。
向こうの答えを正解表として、うちのエンジンに何が足りないかを数で出す。
**うちの検査にもなる** — 実物の様式を何十枚も通すので、こちらの穴も落ちる。

使い方:

    python3 tools/pyoffice_diff.py                     # sample/ と templates/ を全部
    python3 tools/pyoffice_diff.py 様式7.xlsx …        # ファイルを指定
    python3 tools/pyoffice_diff.py --json out.json     # 差の一覧を書き出す
    python3 tools/pyoffice_diff.py --show 20           # 1件ごとに出す差の上限(既定 5)

向こうのサイドカーの場所は環境変数 GENOFFICE で変えられる
(既定 ~/dev/genoffice)。組んでいなければそう言って終わる。

うちのエンジンを動かす python は OFFICEWORK_PYTHON で選ぶ(既定はこの python)。
**repo の直下で素の python を使うと、.venv の officework.pth がソースの方
(エンジンの入っていない officework/)を先に掴む** — wheel を入れた仮想環境を指すこと。

## 何を比べていて、何を比べていないか(2026-08-09 に深くした)

比べる: シート名の並び・使っている範囲・**セルの値そのもの**・**式の文字列
そのもの**・**結合の座標**。

比べない(**比べられない**): 書式・罫線・条件付き書式・入力規則・
名前の定義・固定枠。向こうの `read_range` は返すが、**`officework.sheet` の
Python の窓が返さない**(`Book`/`Sheet` に出ている物が全部)。ここを比べるには
Python の窓を太らせるのではなく、`sheet` クレートを直に使う Rust 側の
書き出しが要る — 本番のサイドカーが Rust なので、そちらが正道。
設計の「次の一手」を参照。
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys
import uuid

GENOFFICE = pathlib.Path(os.environ.get("GENOFFICE", os.path.expanduser("~/dev/genoffice")))
SIDECAR = GENOFFICE / "apps/sheets/native/xlsx-engine/target/release/xlsx-sidecar"

# 端まで読むと大きい帳票で時間が飛ぶので上限を置く。**黙って切らない** —
# 切ったら view に truncated を立て、報告に出す(切った所は「差が無い」ではない)
MAX_ROWS = 5000
MAX_COLS = 256

# 数の食い違いを言う閾。xlsx に書かれた値をどちらもそのまま読むので本来は
# 一致するが、10進の丸めで末尾が揺れることがある
EPS = 1e-9


def a1(row, col):
    """0起点の (行, 列) を A1 に。26列を超えても正しく回す。"""
    s = ""
    c = col
    while True:
        s = chr(ord("A") + c % 26) + s
        c = c // 26 - 1
        if c < 0:
            break
    return f"{s}{row + 1}"


class Sidecar:
    """向こうのサイドカー。stdin/stdout に JSON を1行ずつ。"""

    def __init__(self, path):
        self.p = subprocess.Popen(
            [str(path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )

    def call(self, command, **kw):
        req = {"version": 1, "requestId": str(uuid.uuid4()), "command": command}
        req.update(kw)
        self.p.stdin.write(json.dumps(req) + "\n")
        self.p.stdin.flush()
        line = self.p.stdout.readline()
        if not line:
            err = self.p.stderr.read()
            raise RuntimeError(f"サイドカーが答えません: {err[:400]}")
        return json.loads(line)

    def close(self):
        try:
            self.p.stdin.close()
            self.p.wait(timeout=5)
        except Exception:
            self.p.kill()


def their_view(sc, path):
    """向こうの答えを、比べる形に揃える。"""
    r = sc.call("open", path=str(pathlib.Path(path).resolve()))
    if not r.get("ok"):
        return {"error": r.get("error")}
    meta = r["result"]
    sid = meta.get("sessionId") or meta.get("session_id")
    out = {"sheets": [], "names": len(meta.get("definedNames") or [])}
    # **書式の索引表。** 両側とも styles[] の番号でセルを指すので、
    # 比べるときは番号ではなく**中身に解いてから**見る(番号の振り方は
    # 実装ごとに違ってよい — 揃える意味がない)
    styles = meta.get("styles") or []
    # **サイドカーが「読めなかった」と言った物を落とさない。**
    # Python の窓では `Book.unsupported` に出ていた物で、
    # うちのサイドカーは答えに `xUnsupported` として載せる(向こうには無い欄)
    if meta.get("xUnsupported"):
        out["unsupported"] = meta["xUnsupported"]
    for sh in meta.get("sheets") or []:
        # **範囲はシートの外に出せない**(向こうは "Range is outside the
        # worksheet." で断る)。rowCount / columnCount に丸める
        rows = int(sh.get("rowCount") or 0)
        cols = int(sh.get("columnCount") or 0)
        view = {
            "name": sh.get("name"),
            "extent": [rows, cols],
            "cells": {},
            "merges": [],
            "truncated": rows > MAX_ROWS or cols > MAX_COLS,
        }
        if rows == 0 or cols == 0:
            out["sheets"].append(view)
            continue
        rr = sc.call(
            "read_range",
            sessionId=sid,
            sheetId=sh.get("id"),
            range={
                "startRow": 0,
                "startColumn": 0,
                "endRow": min(rows, MAX_ROWS) - 1,
                "endColumn": min(cols, MAX_COLS) - 1,
            },
        )
        if not rr.get("ok"):
            # **黙って0件にしない。** 断られた理由をそのまま持ち上げる
            view["error"] = rr.get("error")
            out["sheets"].append(view)
            continue
        res = rr["result"]
        # **段階索引。** 向こうは全部読み終わる前に答える。`indexingComplete`
        # は「シート全体の索引が終わったか」で、**要求した範囲が揃っているか
        # ではない**(2026-08-09 に踏んだ — 8行の帳面で indexedThroughRow=7、
        # つまり全部揃っているのに complete は false だった)。
        # 当てになるのは `indexedThroughRow >= 要求の最終行` のほう。
        #
        # ただし副作用がある: 条件付き書式・絞り込み・入力規則・保護は
        # **complete のときだけ返る**(向こうの lib.rs:749-764)。早く読むと
        # 黙って空で返る欄がある — pyoffice はここを真似るか、違えるなら
        # 意識して違える
        want = min(rows, MAX_ROWS) - 1
        got = res.get("indexedThroughRow")
        if got is None or got < want:
            view["partial_through_row"] = got
        if res.get("indexingComplete") is False:
            view["side_fields_withheld"] = True
        for c in res.get("cells") or []:
            v = c.get("value")
            f = c.get("formula")
            # **書式だけのセルは持たない。** 向こうは罫線だけ引いた空欄も
            # 返す(帳票の様式では珍しくない)。こちらは「中身のあるセル」を
            # 見ているので、揃えないと毎回差が出る(2026-08-09 に踏んだ)
            if v in (None, "") and not f:
                continue
            si = c.get("styleIndex")
            st = styles[si] if isinstance(si, int) and si < len(styles) else None
            view["cells"][a1(c["row"], c["column"])] = {"v": v, "f": f, "s": norm_style(st)}
        for m in res.get("merges") or []:
            view["merges"].append(
                f"{a1(m['startRow'], m['startColumn'])}:{a1(m['endRow'], m['endColumn'])}"
            )
        out["sheets"].append(view)
    if sid:
        sc.call("close", sessionId=sid)
    return out


# うちのエンジンは**別プロセスで動かす**。この道具を repo の直下で走らせると、
# .venv の officework.pth が**ソースの officework/(エンジンの入っていない方)**を
# 先に掴んでしまうため(2026-08-09 に踏んだ)。どの python を使うかを選べる形にする。
OUR_PY = os.environ.get("OFFICEWORK_PYTHON", sys.executable)

_OUR_SCRIPT = r"""
import json, pathlib, sys
from officework import sheet

MAX_ROWS, MAX_COLS = %d, %d

def a1(row, col):
    s = ""
    c = col
    while True:
        s = chr(ord("A") + c %% 26) + s
        c = c // 26 - 1
        if c < 0:
            break
    return "%%s%%d" %% (s, row + 1)

path = sys.argv[1]
b = sheet.Book.open(str(pathlib.Path(path).resolve()))
out = {"sheets": [], "names": None, "unsupported": b.unsupported}
for name in b.sheet_names:
    s = b[name]
    rows, cols = s.shape
    view = {
        "name": name,
        "extent": [rows, cols],
        "cells": {},
        "merges": ["%%s:%%s" %% (a, z) for (a, z) in s.merges],
        "truncated": rows > MAX_ROWS or cols > MAX_COLS,
    }
    grid = s.values()
    for r in range(min(rows, MAX_ROWS)):
        row = grid[r] if r < len(grid) else []
        for c in range(min(cols, MAX_COLS)):
            ref = a1(r, c)
            v = row[c] if c < len(row) else None
            # **式のあるセルは、答えが空でも持つ。**「値のあるセルの中で
            # 式を見る」と、空を返す式を取りこぼす(2026-08-09 に踏んだ)
            f = s.formula(ref)
            if v in (None, "") and not f:
                continue
            view["cells"][ref] = {"v": v, "f": f, "s": None}
    out["sheets"].append(view)
print(json.dumps(out, ensure_ascii=False))
""" % (MAX_ROWS, MAX_COLS)


# **うちのサイドカー**(sidecar/ の xlsx-sidecar)。指せば、Python の窓ではなく
# **同じ通信の言葉で**突き合わせる — 本番と同じ道で比べるほうが正しい。
# 未指定なら今までどおり Python の窓を使う(配った wheel の検査になる)
OUR_SIDECAR = os.environ.get("OUR_SIDECAR")


def our_view(path):
    """うちのエンジンの答えを、同じ形に揃える(別プロセス)。"""
    r = subprocess.run(
        [OUR_PY, "-c", _OUR_SCRIPT, str(path)],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if r.returncode != 0:
        last = [l for l in (r.stderr or "").splitlines() if l.strip()]
        return {"error": last[-1] if last else "原因不明"}
    try:
        return json.loads(r.stdout)
    except json.JSONDecodeError:
        return {"error": f"答えが読めません: {r.stdout[:200]}"}


# 書式のうち**比べる欄**。両側が持っていて意味の揃うものだけ。
# 増やすのは、揃わない理由を1つずつ潰してから
STYLE_KEYS = [
    "bold", "italic", "underline", "strikethrough", "wrapText",
    "fontFamily", "fontSize", "fontColor", "fillColor",
    "horizontalAlignment", "verticalAlignment", "numberFormat",
    "borderTop", "borderBottom", "borderLeft", "borderRight",
]


def norm_color(c):
    """色を RRGGBB に揃える。向こうは FFRRGGBB(alpha 付き)で返すことがある。"""
    if not isinstance(c, str):
        return c
    h = c.lstrip("#").upper()
    if len(h) == 8 and h.startswith("FF"):
        h = h[2:]
    return h


def norm_style(st):
    """比べる形に揃える。**無い欄と偽は同じ**(既定)として扱う。"""
    if not st:
        return {}
    o = {}
    for k in STYLE_KEYS:
        v = st.get(k)
        if v in (None, False, ""):
            continue
        # **"General" は「表示形式なし」と同じ。** 向こうは既定を明示して返し、
        # こちらは持たない。意味は同じなので、どちらも無しに寄せる
        if k == "numberFormat" and v == "General":
            continue
        # **既定を明示するか省くかの違いは差ではない。** 向こうは既定も
        # 書いて返し、こちらは省く。xlsx の既定は 横=general・縦=bottom
        if k == "horizontalAlignment" and v == "general":
            continue
        if k == "verticalAlignment" and v == "bottom":
            continue
        if k.endswith("Color"):
            v = norm_color(v)
        elif k.startswith("border") and isinstance(v, dict):
            v = {"style": v.get("style"), "color": norm_color(v.get("color"))}
            # **罫線の色の「自動」。** 原本の <color indexed="64"/> は色ではなく
            # 「自動(前景)」で、向こうは 000000 に解いて返し、こちらは色なしで
            # 返す。画面ではどちらも黒。**黒の明示と自動を区別できないのは
            # 承知の上** — 実物では自動が圧倒的に多く(5万件超)、混ぜたままだと
            # 本物の色の差が埋もれる。実際、青い罫線 73 件がこれに埋もれていた
            if v["color"] in (None, "000000"):
                del v["color"]
        elif k == "fontSize" and isinstance(v, (int, float)):
            v = round(float(v), 2)
        o[k] = v
    return o


def same_value(a, b):
    """値が同じか。数だけは丸めの揺れを許す。"""
    if isinstance(a, bool) or isinstance(b, bool):
        return a is b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) <= EPS * max(1.0, abs(a), abs(b))
    return a == b


def show(x):
    return "(空)" if x is None else repr(x)


# ふりがなに使う字(カタカナ・長音・中黒・空白)。**向こうの欠陥**を見分けるのに使う
_KANA = set(
    "ァアィイゥウェエォオカガキギクグケゲコゴサザシジスズセゼソゾタダチヂッツヅテデトド"
    "ナニヌネノハバパヒビピフブプヘベペホボポマミムメモャヤュユョヨラリルレロヮワヰヱヲンヴ"
    "ーヽヾ・　 "
)


def is_furigana_glued(theirs, ours):
    """向こうの値が「うちの値 + ふりがな」になっていないか。

    向こうは共有文字列の `<rPh>`(ふりがな)の中の `<t>` を本文と**繋げて**返す
    (`実` → `実ジツ`)。**日本語の xlsx でふりがなの無い物のほうが珍しい**ので、
    これを混ぜたままにすると、やることの一覧がふりがなで埋まる。

    **見分けは当て推量なので、二段で確かめる** — ここの形の判定に加えて、
    呼ぶ側がそのファイルの sharedStrings.xml に `<rPh` があることを確かめる。
    """
    if not isinstance(theirs, str) or not isinstance(ours, str):
        return False
    if not theirs.startswith(ours) or len(theirs) == len(ours):
        return False
    tail = theirs[len(ours):]
    return bool(tail) and all(ch in _KANA for ch in tail)


def has_phonetics(path):
    """このファイルの共有文字列にふりがなが入っているか(ZIP を直に覗く)。"""
    try:
        import zipfile

        with zipfile.ZipFile(path) as z:
            return b"<rPh" in z.read("xl/sharedStrings.xml")
    except Exception:
        return False


def diff_sheet(name, t, o, show_n, rph=False):
    """1枚のシートの差。**数ではなく中身**を突き合わせる。

    戻りは (うちの宿題, 向こうの欠陥と分かっている差)。**混ぜない** —
    混ぜると「直すことの一覧」が汚れる。
    """
    d = []
    theirs_bug = []
    if t.get("error"):
        return [f"[{name}] 向こうが断った: {t['error']}"]
    if t.get("extent") != o.get("extent"):
        d.append(f"[{name}] 使っている範囲: 向こう {t['extent']} / うち {o['extent']}")
    if "partial_through_row" in t:
        d.append(
            f"[{name}] **向こうは {t['partial_through_row']} 行までしか索引していない** — "
            f"この先の一致は当てにならない"
        )
    if t.get("truncated") or o.get("truncated"):
        d.append(f"[{name}] 大きすぎて {MAX_ROWS}行×{MAX_COLS}列で切った — 先は見ていない")

    tc, oc = t.get("cells") or {}, o.get("cells") or {}
    only_t = sorted(set(tc) - set(oc))
    only_o = sorted(set(oc) - set(tc))
    if only_t:
        d.append(
            f"[{name}] うちに無いセル {len(only_t)} 個: "
            + ", ".join(f"{k}={show(tc[k]['v'])}" for k in only_t[:show_n])
        )
    if only_o:
        d.append(
            f"[{name}] 向こうに無いセル {len(only_o)} 個: "
            + ", ".join(f"{k}={show(oc[k]['v'])}" for k in only_o[:show_n])
        )

    vbad, fbad, kana, sbad = [], [], [], []
    for k in sorted(set(tc) & set(oc)):
        if not same_value(tc[k]["v"], oc[k]["v"]):
            line = f"{k}: 向こう {show(tc[k]['v'])} / うち {show(oc[k]['v'])}"
            if rph and is_furigana_glued(tc[k]["v"], oc[k]["v"]):
                kana.append(line)
            else:
                vbad.append(line)
        if (tc[k]["f"] or "") != (oc[k]["f"] or ""):
            fbad.append(f"{k}: 向こう {show(tc[k]['f'])} / うち {show(oc[k]['f'])}")
        ts, os_ = tc[k].get("s") or {}, oc[k].get("s") or {}
        if ts != os_:
            # **違う欄だけ言う。** 書式まるごと並べると読めない
            keys = sorted(set(ts) | set(os_))
            d2 = [f"{x}: 向こう {ts.get(x)!r} / うち {os_.get(x)!r}" for x in keys
                  if ts.get(x) != os_.get(x)]
            sbad.append(f"{k}: " + " ".join(d2))
    if kana:
        theirs_bug.append(
            f"[{name}] ふりがなの連結 {len(kana)} 個(**向こうの欠陥**。うちが正しい): "
            + " / ".join(kana[:show_n])
        )
    if vbad:
        d.append(f"[{name}] 値が違うセル {len(vbad)} 個: " + " / ".join(vbad[:show_n]))
    if fbad:
        d.append(f"[{name}] 式が違うセル {len(fbad)} 個: " + " / ".join(fbad[:show_n]))
    if sbad:
        d.append(f"[{name}] 書式が違うセル {len(sbad)} 個: " + " / ".join(sbad[:show_n]))

    tm, om = set(t.get("merges") or []), set(o.get("merges") or [])
    if tm != om:
        d.append(
            f"[{name}] 結合が違う: うちに無い {sorted(tm - om)[:show_n]} / "
            f"向こうに無い {sorted(om - tm)[:show_n]}"
        )
    return d, theirs_bug


def compare(path, sc, show_n, ours_sc=None):
    theirs = their_view(sc, path)
    # **同じ抽出で揃える。** 通信の言葉が同じなら、取り出す道も同じでよい
    ours = their_view(ours_sc, path) if ours_sc is not None else our_view(path)
    diffs, theirs_bug = [], []
    if "error" in theirs or "error" in ours:
        diffs.append(f"読めない — 向こう: {theirs.get('error')} / うち: {ours.get('error')}")
        return theirs, ours, diffs, theirs_bug
    rph = has_phonetics(path)
    tn = [s["name"] for s in theirs["sheets"]]
    on = [s["name"] for s in ours["sheets"]]
    if tn != on:
        diffs.append(f"シートの並びが違う: 向こう {tn} / うち {on}")
    # 名前の並びが違っても、同じ名前のシート同士は比べる(片方だけの物は上で出た)
    ot = {s["name"]: s for s in ours["sheets"]}
    for t in theirs["sheets"]:
        o = ot.get(t["name"])
        if o is None:
            continue
        d, tb = diff_sheet(t["name"], t, o, show_n, rph)
        diffs += d
        theirs_bug += tb
    if ours.get("unsupported"):
        diffs.append(f"うちが読めなかった物: {ours['unsupported']}")
    return theirs, ours, diffs, theirs_bug


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="*")
    ap.add_argument("--json", help="差の一覧の書き出し先")
    ap.add_argument("--show", type=int, default=5, help="1件ごとに出す差の上限")
    ap.add_argument("--ours-sidecar", help="うちの側もこのサイドカーで比べる(既定は Python の窓)")
    a = ap.parse_args()

    if not SIDECAR.exists():
        sys.exit(
            f"向こうのサイドカーがありません: {SIDECAR}\n"
            f"  cd {SIDECAR.parents[1]} && cargo build --release"
        )

    files = [pathlib.Path(f) for f in a.files]
    if not files:
        for d in ("sample", "templates"):
            files += sorted(pathlib.Path(d).glob("*.xlsx"))
    if not files:
        sys.exit("比べる xlsx がありません")

    ours_sc = None
    if a.ours_sidecar or OUR_SIDECAR:
        p = pathlib.Path(a.ours_sidecar or OUR_SIDECAR)
        if not p.exists():
            sys.exit(f"うちのサイドカーがありません: {p}")
        ours_sc = Sidecar(p)
        print(f"うちの側もサイドカーで比べる: {p}\n")

    sc = Sidecar(SIDECAR)
    report = []
    try:
        for f in files:
            theirs, ours, diffs, theirs_bug = compare(f, sc, a.show, ours_sc)
            report.append(
                {
                    "file": str(f),
                    "diffs": diffs,
                    "theirs_bug": theirs_bug,
                    "theirs": theirs,
                    "ours": ours,
                }
            )
            # **△ = 差はあるが、向こうの欠陥と分かっている物だけ。**
            # ○ と混ぜると宿題が水増しされ、× と混ぜると直せない物を追いかける
            mark = "×" if diffs else ("△" if theirs_bug else "○")
            print(f"{mark} {f}")
            for d in diffs:
                print(f"    {d}")
            for d in theirs_bug:
                print(f"    ~ {d}")
    finally:
        sc.close()
        if ours_sc is not None:
            ours_sc.close()

    n = sum(1 for r in report if r["diffs"])
    m = sum(1 for r in report if not r["diffs"] and r["theirs_bug"])
    cells = sum(
        len(s.get("cells") or {}) for r in report for s in (r["theirs"].get("sheets") or [])
    )
    print(f"\n{len(report)} 枚中 {n} 枚で差が出ました(比べたセル {cells} 個)")
    if m:
        print(f"  ほかに {m} 枚は △ — 向こうの欠陥と分かっている差だけ(うちの宿題ではない)")
    # **緑を大きく見せない。** 何を見ていないかを毎回言う
    if a.ours_sidecar or OUR_SIDECAR:
        print("  ※ うちの側はサイドカー。埋めていない欄は答えの xNotFilled に出る")
    else:
        print(
            "  比べていない: 書式・罫線・条件付き書式・入力規則・名前の定義・固定枠 "
            "(Python の窓が返さない — 設計の「次の一手」)"
        )
    if a.json:
        pathlib.Path(a.json).write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(f"書き出し: {a.json}")
    return 1 if n else 0


if __name__ == "__main__":
    sys.exit(main())
