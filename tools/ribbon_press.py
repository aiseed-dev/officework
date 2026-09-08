#!/usr/bin/env python3
"""**リボンのボタンを受け口から全部押して、動くかを確かめる道具**(2026-09-09
発注者「OfficeWork のリボンのボタンが動作するかどうか確認して、動作しなければ
修正して」)。

Mac では画面のクリックを外から送れない(補助アクセスの許可が要る)ので、
アプリの受け口(unix ソケット)の `press` でボタンを id で押し、`ui_state` と
`ping` で「落ちていない・何かが起きた」を見ます。tools/ribbon_sweep.py
(X11 で実際にクリックする道具)の Mac 版です。

    python3 tools/ribbon_press.py                # 起動中の officework に繋ぐ
    python3 tools/ribbon_press.py --app calc     # calc.sock に繋ぐ
    python3 tools/ribbon_press.py --only bold italic

やること: face/src/ribbon.rs の押せるボタン(`c(`/`t(`/`m(`)の id を集め、
いま前に出ている画面(表か文書か)で効く物を順に押す。押すたびに

1. 受け口が答える(落ちていない)
2. `ui_state` か状態行が変わった、または状態行に「まだ」「未対応」の断りが無い
3. `escape` を押して、開いた物を閉じる

を見て、結果を1行ずつ出します。最後に「押した数・何も起きなかった物・
断られた物・落ちた所」をまとめます。
"""
import argparse
import json
import os
import re
import socket
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def sock_path(app):
    base = os.environ.get("XDG_RUNTIME_DIR")
    if base:
        p = os.path.join(base, "officework", f"{app}.sock")
        if len(p) <= 90:
            return p
    # Rust 側は /proc/self が無い Mac では uid を 0 と見る
    uid = 0 if not os.path.exists("/proc/self") else os.getuid()
    tmp = os.environ.get("TMPDIR", "/tmp")
    return os.path.join(tmp, f"officework-{uid}", f"{app}.sock")


def rpc(path, obj, timeout=20.0):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(timeout)
    s.connect(path)
    s.sendall((json.dumps(obj, ensure_ascii=False) + "\n").encode())
    buf = b""
    while not buf.endswith(b"\n"):
        chunk = s.recv(65536)
        if not chunk:
            break
        buf += chunk
    s.close()
    return json.loads(buf.decode() or "{}")


def ribbon_ids(pane):
    """その画面(doc / sheet)で押せるボタンの id(ui/gen_ribbon.py の READY から)"""
    import ast
    src = open(os.path.join(ROOT, "ui", "gen_ribbon.py"), encoding="utf-8").read()
    m = re.search(r"READY = (\{.*?\n\})\n", src, re.S)
    ready = ast.literal_eval(m.group(1))
    key = "writer" if pane == "doc" else "calc"
    ids, seen = [], set()
    for v in ready[key].values():
        if v not in seen:
            seen.add(v)
            ids.append(v)
    return ids


# 押さない物: ファイルの小窓を開く・外のアプリを起動する・ファイルを作る・
# モデルが要る・アプリを終える。押すと点検が止まるか、後片付けが要る
SKIP = {
    "open", "save", "saveas", "print", "pdf", "quit", "close", "new", "blankpage",
    "insimage", "insertimage", "text-from-file", "py-new", "py-edit", "py-line",
    "py-folder", "py-run", "py-list", "py-calc", "plug-macros", "plug-manage", "terminal",
    "ai-where", "ai-summary", "ai-rewrite", "ai-polite", "ai-plain", "ai-translate",
    "ai-furigana", "ai-continue", "ai-table", "ai-ask", "ai-macro", "coauth-mode",
    "co-chat", "co-history", "prot-encrypt", "prot-sign", "macro-run", "rec-toggle",
    "python", "from-file", "insert-file", "hyperlink", "darkmode",
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--app", default="officework")
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--skip", nargs="*", default=[])
    a = ap.parse_args()
    path = sock_path(a.app)
    if not os.path.exists(path):
        raise SystemExit(f"受け口が無い: {path}(アプリを起動してください)")
    pong = rpc(path, {"cmd": "ping"})
    print("ping:", pong)
    pane = pong.get("showing", "sheet")
    ids = a.only or [i for i in ribbon_ids(pane) if i not in SKIP and i not in a.skip]
    print(f"pane={pane} buttons={len(ids)}")
    before = rpc(path, {"cmd": "ui_state"})
    print("state:", json.dumps(before, ensure_ascii=False)[:200])
    nothing, refused, dead, ok = [], [], [], []
    for i in ids:
        t0 = time.time()
        try:
            r = rpc(path, {"cmd": "press", "id": i})
        except Exception as e:  # noqa: BLE001
            dead.append((i, str(e)))
            print(f"× {i}: 受け口が答えない: {e}")
            break
        if not r.get("ok"):
            refused.append((i, r.get("error") or r))
            print(f"- {i}: 断られた: {r.get('error') or r}")
            continue
        try:
            after = rpc(path, {"cmd": "ui_state"})
        except Exception as e:  # noqa: BLE001
            dead.append((i, str(e)))
            print(f"× {i}: 押した後に受け口が答えない: {e}")
            break
        changed = json.dumps(after, ensure_ascii=False) != json.dumps(before, ensure_ascii=False)
        status = str(after.get("status", ""))
        mada = any(k in status for k in ("まだ", "未対応", "できません", "not yet", "not available"))
        mark = "+" if changed and not mada else ("~" if mada else "?")
        print(f"{mark} {i}: {'変わった' if changed else '変わらない'} {status[:60]!r} {time.time() - t0:.1f}s")
        (ok if changed and not mada else nothing).append((i, status))
        # 開いた物を閉じる
        try:
            rpc(path, {"cmd": "press", "id": "escape"})
            rpc(path, {"cmd": "press", "id": "escape"})
            before = rpc(path, {"cmd": "ui_state"})
        except Exception as e:  # noqa: BLE001
            dead.append((i, f"escape の後に答えない: {e}"))
            print(f"× {i}: escape の後に受け口が答えない: {e}")
            break
    print(f"\n押した {len(ok) + len(nothing)} / 断られた {len(refused)} / 何も起きない {len(nothing)} / 落ちた {len(dead)}")
    for i, s in nothing:
        print(f"  何も起きない: {i} {s[:60]!r}")
    for i, e in refused:
        print(f"  断られた: {i} {e}")
    for i, e in dead:
        print(f"  落ちた: {i} {e}")
    sys.exit(1 if dead else 0)


if __name__ == "__main__":
    main()
