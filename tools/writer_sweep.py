#!/usr/bin/env python3
"""writer のリボンの全ボタンを実機で一巡して点検する。

calc の tools/ribbon_sweep.py の writer 版。writer には socket の受け口が
無いので、様子は OFFICEWORK_UI_DUMP の ui.json から読む(writer_shot.py と
同じ道)。ui.json には一覧(ドロップダウン)の位置が入っていないため、
calc 版の「押したボタンの真下か」の検査はここでは出来ない。見るのは:

1. 落ちない
2. 押したあとも応える(ui.json が読み直せる = 描けている)
3. Esc のあとも応える(開いた物を引きずって固まらない)

「何かが起きたか」は状態行と選択と段の変化を**記録するだけ**にする —
calc 版で誤報が多かった検査なので、落とす材料にはしない(--strict で落とす)。

使い方:

    python3 tools/writer_sweep.py                 # ぜんぶの段
    python3 tools/writer_sweep.py --tabs 1 2      # 段を選ぶ

前提は writer_shot.py と同じ(X11・python-xlib・cargo build --release -p writer)。
ファイルを選ぶ小窓は、XDG_RUNTIME_DIR が偽物で rfd のポータルに届かない
ため**そもそも開かない**(writer_shot.py の註)。だから calc 版のような
SKIP の表は要らない。
"""

import argparse
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tools"))
import writer_shot as ws  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tabs", nargs="*", type=int, help="段の番号(既定: ファイル以外ぜんぶ)")
    ap.add_argument("--strict", action="store_true", help="「押して何か起きたか」でも落とす")
    ap.add_argument("--shots", default="/tmp/writer-sweep")
    a = ap.parse_args()

    app = ws.W(a.shots)
    out = []
    total = 0
    try:
        u = app.ui()
        tabs = sorted(
            int(b["id"][4:]) for b in u["boxes"] if b["id"].startswith("@tab")
        )
        todo = a.tabs if a.tabs else [t for t in tabs if t != 0]
        for t in todo:
            print(f"-- 段 {t} …", flush=True)
            try:
                u = app.tab(t)
            except SystemExit as e:
                # 文脈の段(いま出ていない)はここに来る。正直に飛ばす
                print(f"   (出ていない段: {e})")
                continue
            ids = [b["id"] for b in sorted(u["boxes"], key=lambda b: (b["y"], b["x"]))
                   if not b["id"].startswith("@tab")]
            for bid in ids:
                u = app.ui(want_boxes=True)
                if u["tab"] != t:
                    # 前のボタンで段が動いた(f-tpl 等)。戻して続ける
                    try:
                        u = app.tab(t)
                    except SystemExit:
                        out.append((bid, "段", f"段 {t} に戻れない"))
                        break
                b = next((x for x in u["boxes"] if x["id"] == bid), None)
                if b is None:
                    continue  # 段の姿が変わって消えたボタン(トグルの相方など)
                before = (u.get("status"), u.get("sel"), u.get("tab"))
                try:
                    app.click(b["x"] + b["w"] / 2, b["y"] + b["h"] / 2, 0.8)
                except SystemExit as e:
                    total += 1
                    app.log.flush()
                    tail = open(app.log.name, encoding="utf-8", errors="replace").read()[-400:]
                    out.append((bid, "窓が消えた", f"{e} / log: {tail.strip()[-300:]}"))
                    print(f"   [窓が消えた] {bid}")
                    return report(out, total)
                total += 1
                if app.proc.poll() is not None:
                    app.log.flush()
                    tail = open(app.log.name, encoding="utf-8", errors="replace").read()[-400:]
                    out.append((bid, "落ちた", tail.strip()[-300:]))
                    print(f"   [落ちた] {bid}")
                    return report(out, total)
                try:
                    after = app.ui(want_boxes=False)
                except SystemExit:
                    out.append((bid, "固まった", "押したあと ui.json が読めない"))
                    app.shot(f"katamari-{bid}")
                    return report(out, total)
                if a.strict and (after.get("status"), after.get("sel"), after.get("tab")) == before:
                    out.append((bid, "無反応", "押しても状態行も選択も段も変わらない"))
                app.key("Escape", 0.4)
                app.key("Escape", 0.3)
                try:
                    app.ui(want_boxes=False)
                except SystemExit:
                    out.append((bid, "固まった", "Esc のあと ui.json が読めない"))
                    app.shot(f"esc-katamari-{bid}")
                    return report(out, total)
    finally:
        app.close()
    return report(out, total)


def report(out, total):
    print(f"\n押したボタン: {total}")
    if not out:
        print("しくじりなし。")
        return 0
    print(f"しくじり: {len(out)} 件\n")
    for bid, kind, msg in out:
        print(f"  [{kind}] {bid}: {msg}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
