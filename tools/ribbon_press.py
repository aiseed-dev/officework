#!/usr/bin/env python3
"""**A tool that presses every ribbon button through the API and checks that it works.**
The owner asked on 2026-09-09 for the ribbon buttons of OfficeWork to be checked, and
fixed where they do not work.

On the Mac, clicks cannot be sent to the screen from outside without permission for
assistive access. So this tool presses a button by id with `press` on the app's API (a
unix socket) and uses `ui_state` and `ping` to see that the app has not crashed and that
something happened. It is the Mac counterpart of tools/ribbon_sweep.py, which really
clicks through X11.

    python3 tools/ribbon_press.py                # connect to a running officework
    python3 tools/ribbon_press.py --app calc     # connect to calc.sock
    python3 tools/ribbon_press.py --only bold italic

What it does: it collects the ids of the buttons in face/src/ribbon.rs that can be
pressed (`c(`, `t(` and `m(`), and presses the ones that work on the screen that is in
front (the sheet or the document) one after another. For each press it checks

1. the API answers (the app has not crashed)
2. `ui_state` or the status bar changed, or the status bar does not say the command is
   not supported yet
3. `escape` is pressed to close whatever opened

and prints one line per result. At the end it sums up how many buttons were pressed,
which did nothing, which were refused, and where the app crashed.
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
    """その画面(doc / sheet)で押せるボタンの id。

    **face/src/ribbon.rs から読みます**(2026-09-09 に直しました)。前は
    ui/gen_ribbon.py の READY を読んでいましたが、あの表は生成の元の一部で、
    後から EXTRA_CMDS で足したボタン(コピー・書式のコピー・並べ替え・
    ピボットの一部など)が入っていません。表の画面で押せる 191 個のうち
    42 個が点検から漏れていました。アプリが実際に見るのは ribbon.rs なので、
    そちらを読みます。

    その画面の表(WRITER か CALC)にあるボタンだけを返します。もう片方に
    しか無いボタンは、この画面では灰色なので押しません。タイトルバーの4つ
    (face/src/tabs.rs の TITLEBAR)も押せるので、頭に足します。
    """
    src = open(os.path.join(ROOT, "face", "src", "ribbon.rs"), encoding="utf-8").read()
    konst = "WRITER" if pane == "doc" else "CALC"
    i = src.index(f"pub const {konst}: &[Tab] = &[")
    mine = re.findall(r'\n\s*[ctm]\("([^"]+)"', src[i:src.index("\n];\n", i)])

    tb = open(os.path.join(ROOT, "face", "src", "tabs.rs"), encoding="utf-8").read()
    m = re.search(r"pub const TITLEBAR: &\[&str\] = &\[(.*?)\];", tb, re.S)
    ids = re.findall(r'"([^"]+)"', m.group(1)) if m else []
    for i in mine:
        if i not in ids:
            ids.append(i)
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
    # ファイルの小窓(rfd)を開く物。小窓が出ている間は受け口が答えない
    "inschart", "smartpicker", "insequation-image", "prot-doc",
    # 表の画面でファイル選択の窓を開く2つ(2026-09-09 に踏みました)。
    # 窓が出たまま主スレッドが止まり、以降のボタンが全部「応じません」に
    # なります。calc/src/tests.rs の DIALOG と同じ顔ぶれです
    "data-from-text", "data-external-links",
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
    nothing, refused, dead, ok, grey = [], [], [], [], []
    for i in ids:
        t0 = time.time()
        try:
            r = rpc(path, {"cmd": "press", "id": i})
        except Exception as e:  # noqa: BLE001
            dead.append((i, str(e)))
            print(f"× {i}: 受け口が答えない: {e}")
            break
        if not r.get("ok"):
            err = str(r.get("error") or r.get("err") or r)
            if "no such ready button" in err:
                grey.append(i)
                print(f"  {i}: この画面では灰色")
                continue
            refused.append((i, err))
            print(f"- {i}: 断られた: {err}")
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
    print(f"\n押した {len(ok) + len(nothing)} / 灰色 {len(grey)} / 断られた {len(refused)} / 何も起きない {len(nothing)} / 落ちた {len(dead)}")
    if grey:
        print("  灰色(この画面では効かない):", " ".join(grey))
    for i, s in nothing:
        print(f"  何も起きない: {i} {s[:60]!r}")
    for i, e in refused:
        print(f"  断られた: {i} {e}")
    for i, e in dead:
        print(f"  落ちた: {i} {e}")
    sys.exit(1 if dead else 0)


if __name__ == "__main__":
    main()
