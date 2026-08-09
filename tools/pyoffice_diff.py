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
        sid_sheet = sh.get("id") or sh.get("sheetId")
        view = {"name": sh.get("name"), "cells": 0, "formulas": 0}
        rr = sc.call(
            "read_range",
            sessionId=sid,
            sheetId=sid_sheet,
            range={"startRow": 0, "startColumn": 0, "endRow": 200, "endColumn": 40},
        )
        if rr.get("ok"):
            res = rr["result"]
            cells = res.get("cells") or []
            view["cells"] = len(cells)
            view["merges"] = len(res.get("merges") or [])
            view["formulas"] = sum(1 for c in cells if c.get("formula"))
        out["sheets"].append(view)
    if sid:
        sc.call("close", sessionId=sid)
    return out


def our_view(path):
    """うちのエンジンの答えを、同じ形に揃える。"""
    try:
        from officework import sheet
    except ImportError as e:
        return {"error": f"officework が入っていません: {e}"}
    try:
        b = sheet.Book.open(str(pathlib.Path(path).resolve()))
    except Exception as e:
        return {"error": str(e)}
    out = {"sheets": [], "names": 0, "unsupported": b.unsupported}
    for name in b.sheet_names:
        s = b[name]
        rows, cols = s.shape
        cells = 0
        formulas = 0
        for r in range(min(rows, 200)):
            for c in range(min(cols, 40)):
                a1 = f"{chr(65 + c) if c < 26 else 'A' + chr(65 + c - 26)}{r + 1}"
                v = s[a1]
                if v not in (None, ""):
                    cells += 1
                    if s.formula(a1):
                        formulas += 1
        out["sheets"].append({"name": name, "cells": cells, "formulas": formulas})
    return out


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
