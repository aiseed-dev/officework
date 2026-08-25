#!/usr/bin/env python3
"""鍵の英語化 — t!/tf!/item! の鍵を ja から en へ裏返す機械。"""
import json, pathlib, re, sys

ROOT = pathlib.Path("/home/dev/dev/officework")
HERE = pathlib.Path(__file__).parent
SRC_DIRS = ["calc/src", "writer/src", "face/src", "ui/src", "ops/src",
            "officework/src", "sheet/src", "engine/src", "lang/src",
            "paper/src", "pyrun/src", "ooxml/src", "pysheet/src", "sidecar/src"]

def unescape(s):
    return s.replace('\\"', '"').replace("\\n", "\n").replace("\\\\", "\\")

def escape(s):
    return s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")

def load_pairs():
    t = (ROOT / "lang/src/i18n_en.rs").read_text(encoding="utf-8")
    raw = re.findall(r'\("((?:[^"\\]|\\.)*)",\s*"((?:[^"\\]|\\.)*)"\)', t)
    return [(unescape(a), unescape(b)) for a, b in raw]

def load_fixes():
    m, canon = {}, {}
    for line in (HERE / "flip_en_fixes.tsv").read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        ja, en = cols[0], cols[1]
        m[ja] = en
        if len(cols) > 2 and cols[2].strip():
            canon[en] = cols[2]
    return m, canon

def mapping():
    fixes, canon = load_fixes()
    m = {}
    for ja, en in load_pairs():
        m[ja] = fixes.get(ja, en)
    inv = {}
    for ja, en in m.items():
        inv.setdefault(en, []).append(ja)
    bad = {e: js for e, js in inv.items() if len(js) > 1 and e not in canon}
    for e, js in bad.items():
        print(f"!! 統合の正の日本語が無い: {e!r} ← {js}")
    if bad:
        sys.exit(1)
    return m, canon

def rewrite_code(m, dry):
    pat = re.compile(r'(ui::(?:t|tf|item)!\(\s*)"((?:[^"\\]|\\.)*)"')
    n_hit = n_miss = 0
    misses = {}
    for d in SRC_DIRS:
        for p in (ROOT / d).rglob("*.rs"):
            t = p.read_text(encoding="utf-8")
            out, last = [], 0
            changed = False
            for mo in pat.finditer(t):
                ja = unescape(mo.group(2))
                if ja in m:
                    en = m[ja]
                    out.append(t[last:mo.start()])
                    out.append(mo.group(1) + '"' + escape(en) + '"')
                    last = mo.end()
                    n_hit += 1
                    changed = True
                else:
                    n_miss += 1
                    misses.setdefault(ja, str(p.relative_to(ROOT)))
            out.append(t[last:])
            if changed and not dry:
                p.write_text("".join(out), encoding="utf-8")
    print(f"code: 書き替え {n_hit} / 表に無い鍵 {n_miss}")
    for ja, w in list(misses.items())[:15]:
        print(f"   表に無い: {ja[:60]!r} … {w}")
    return n_miss

if __name__ == "__main__":
    dry = "--go" not in sys.argv
    m, canon = mapping()
    print(f"対 {len(m)} 句 → en 鍵 {len(set(m.values()))}(統合 {len(m) - len(set(m.values()))})")
    rewrite_code(m, dry)
    if dry:
        print("(--go で書き込む)")
