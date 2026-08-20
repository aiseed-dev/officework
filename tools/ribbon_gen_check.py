#!/usr/bin/env python3
"""**生成スクリプトが実物を再現できるかを見る**(B-1。2026-08-21)。

`ui/gen_ribbon.py` を回して、その出力と `face/src/ribbon.rs` を突き合わせます。
見るのは *id・札・絵・書き方(押す/入切/灰色)・並び*。合っていなければ止めます。

## なぜ上書きではなく検査なのか

実物の表には**決めの理由が註として書いてあります**(「ホームの Σ はオート
SUM。本家のホームの Σ はそちら」「Python はブックと切り離した」など)。
生成し直して上書きすると、その 40 行あまりが消えます。

だから実物は手で持ったまま、**離れたら止まる**形にしました。
ずれたときは、どちらが正しいかを人が決めます。

* 本家の並びが変わった → 生成スクリプトの出力が正。実物を直す
* うちで意図して変えた → 生成スクリプトの表(EXTRA_CMDS・並べ替え・言い換え)に
  その意図を書く

## 使い方

    python3 tools/ribbon_gen_check.py
"""
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import ribbon_parse as R  # noqa: E402


def main() -> int:
    r = subprocess.run(
        [sys.executable, str(ROOT / "ui" / "gen_ribbon.py")],
        capture_output=True, text=True, cwd=ROOT, timeout=600,
    )
    if r.returncode != 0:
        print("::error::生成スクリプトが落ちました")
        print(r.stderr[-2000:])
        return 1
    with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False, encoding="utf-8") as f:
        f.write(r.stdout)
        tmp = pathlib.Path(f.name)
    try:
        gen = R.tables_or_die(tmp)
    finally:
        tmp.unlink(missing_ok=True)
    real = R.tables_or_die(ROOT / "face" / "src" / "ribbon.rs")

    違い = []
    for 表 in ("WRITER", "CALC"):
        g, c = gen[表], real[表]
        gn = [t.name for t in g]
        cn = [t.name for t in c]
        if gn != cn:
            違い.append(f"{表}: タブの並びが違います\n  生成 {gn}\n  実物 {cn}")
            continue
        for t1, t2 in zip(g, c):
            a = [(x.kind, x.id, x.label, x.icon) for x in t1.cmds]
            b = [(x.kind, x.id, x.label, x.icon) for x in t2.cmds]
            if a == b:
                continue
            違い.append(f"{表} / {t1.name}: 生成 {len(a)} 個 / 実物 {len(b)} 個")
            for k in range(max(len(a), len(b))):
                x = a[k] if k < len(a) else None
                y = b[k] if k < len(b) else None
                if x != y:
                    違い.append(f"    {k} 番目: 生成 {x} / 実物 {y}")

    if 違い:
        print("::error::生成スクリプトの出力が実物と違います")
        for d in 違い[:40]:
            print(d)
        print()
        print("どちらが正しいかを決めてから直してください:")
        print("  本家が変わった  → 実物(face/src/ribbon.rs)を直す")
        print("  うちで変えた    → ui/gen_ribbon.py の表に意図を書く")
        print("    EXTRA_CMDS(足す) / 並べ替え(動かす) / 外す / 言い換え / 絵の差し替え")
        return 1
    n = sum(len(t.cmds) for 表 in ("WRITER", "CALC") for t in real[表])
    print(f"生成スクリプトは実物を再現できます(タブ {len(real['WRITER']) + len(real['CALC'])}・ボタン {n})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
