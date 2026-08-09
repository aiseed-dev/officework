#!/usr/bin/env python3
"""genoffice のサイドカーと officework のエンジンに同じ xlsx を通し、答えを比べる。

**実装から入らないための道具**(SEKKEI docs/sekkei/pyoffice.ja.md)。
向こうの答えを正解表として、うちのエンジンに何が足りないかを数で出す。
**うちの検査にもなる** — 実物の様式を何十枚も通すので、こちらの穴も落ちる。

使い方:

    python3 tools/pyoffice_diff.py                     # sample/ と templates/ を全部
    python3 tools/pyoffice_diff.py 様式7.xlsx …        # ファイルを指定
    python3 tools/pyoffice_diff.py --json out.json     # 差の一覧を書き出す

向こうのサイドカーの場所は環境変数 GENOFFICE で変えられる
(既定 ~/dev/genoffice)。組んでいなければそう言って終わる。

うちのエンジンを動かす python は OFFICEWORK_PYTHON で選ぶ(既定はこの python)。
**repo の直下で素の python を使うと、.venv の officework.pth がソースの方
(エンジンの入っていない officework/)を先に掴む** — wheel を入れた仮想環境を指すこと。
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
    """向こうの答えから、比べたい所だけ取り出す。"""
    r = sc.call("open", path=str(pathlib.Path(path).resolve()))
    if not r.get("ok"):
        return {"error": r.get("error")}
    meta = r["result"]
    sid = meta.get("sessionId") or meta.get("session_id")
    out = {"sheets": [], "names": len(meta.get("definedNames") or [])}
    for sh in meta.get("sheets") or []:
        view = {"name": sh.get("name"), "cells": 0, "formulas": 0}
        # **範囲はシートの外に出せない**(向こうは "Range is outside the
        # worksheet." で断る)。rowCount / columnCount に丸める
        rows = int(sh.get("rowCount") or 0)
        cols = int(sh.get("columnCount") or 0)
        if rows == 0 or cols == 0:
            view["note"] = "空のシート"
            out["sheets"].append(view)
            continue
        rr = sc.call(
            "read_range",
            sessionId=sid,
            sheetId=sh.get("id"),
            range={
                "startRow": 0,
                "startColumn": 0,
                "endRow": rows - 1,
                "endColumn": cols - 1,
            },
        )
        if not rr.get("ok"):
            # **黙って0件にしない。** 断られた理由をそのまま持ち上げる
            view["error"] = rr.get("error")
        else:
            res = rr["result"]
            cells = res.get("cells") or []
            # **書式だけのセルは数えない。** 向こうは罫線だけ引いた空欄も
            # 返す(帳票の様式では珍しくない)。こちらは「中身のあるセル」を
            # 数えているので、揃えないと毎回差が出る(2026-08-09 に踏んだ)
            has = [
                c
                for c in cells
                if c.get("value") not in (None, "") or c.get("formula")
            ]
            view["cells"] = len(has)
            view["fmt_only"] = len(cells) - len(has)
            view["merges"] = len(res.get("merges") or [])
            view["formulas"] = sum(1 for c in has if c.get("formula"))
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
path = sys.argv[1]
b = sheet.Book.open(str(pathlib.Path(path).resolve()))
out = {"sheets": [], "names": 0, "unsupported": b.unsupported}
for name in b.sheet_names:
    s = b[name]
    rows, cols = s.shape
    cells = formulas = 0
    for r in range(min(rows, 200)):
        for c in range(min(cols, 40)):
            a1 = (chr(65 + c) if c < 26 else "A" + chr(65 + c - 26)) + str(r + 1)
            # **式のあるセルは、答えが空でも数える。**「値のあるセルの中で
            # 式を数える」と、空を返す式を取りこぼす(2026-08-09 に踏んだ)
            f = s.formula(a1)
            if f:
                formulas += 1
            if s[a1] not in (None, "") or f:
                cells += 1
    out["sheets"].append({"name": name, "cells": cells, "formulas": formulas})
print(json.dumps(out, ensure_ascii=False))
"""


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


def compare(path, sc):
    theirs = their_view(sc, path)
    ours = our_view(path)
    diffs = []
    if "error" in theirs or "error" in ours:
        diffs.append(f"読めない — 向こう: {theirs.get('error')} / うち: {ours.get('error')}")
        return theirs, ours, diffs
    tn = [s["name"] for s in theirs["sheets"]]
    on = [s["name"] for s in ours["sheets"]]
    if tn != on:
        diffs.append(f"シートの並びが違う: 向こう {tn} / うち {on}")
    for t, o in zip(theirs["sheets"], ours["sheets"]):
        if t.get("error"):
            diffs.append(f"[{t['name']}] 向こうが断った: {t['error']}")
            continue
        if t["cells"] != o["cells"]:
            diffs.append(f"[{t['name']}] 中身のあるセルの数: 向こう {t['cells']} / うち {o['cells']}")
        if t["formulas"] != o["formulas"]:
            diffs.append(f"[{t['name']}] 式のセルの数: 向こう {t['formulas']} / うち {o['formulas']}")
    if ours.get("unsupported"):
        diffs.append(f"うちが読めなかった物: {ours['unsupported']}")
    return theirs, ours, diffs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="*")
    ap.add_argument("--json", help="差の一覧の書き出し先")
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

    sc = Sidecar(SIDECAR)
    report = []
    try:
        for f in files:
            theirs, ours, diffs = compare(f, sc)
            report.append({"file": str(f), "diffs": diffs, "theirs": theirs, "ours": ours})
            mark = "○" if not diffs else "×"
            print(f"{mark} {f}")
            for d in diffs:
                print(f"    {d}")
    finally:
        sc.close()

    n = sum(1 for r in report if r["diffs"])
    print(f"\n{len(report)} 枚中 {n} 枚で差が出ました")
    if a.json:
        pathlib.Path(a.json).write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(f"書き出し: {a.json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
