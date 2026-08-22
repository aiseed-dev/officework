#!/usr/bin/env python3
"""**統合したアプリ(officework)を実機で起こして、押して、撮る道具。**

`ribbon_sweep.py` は calc 単体、`writer_shot.py` は writer 単体を相手に
します。配るのは統合した `officework` 1本(SEKKEI 段11)なので、それを
相手にする道具がありませんでした。2026-08-22 までは、画面を直すたびに
同じ仕掛けを使い捨ての場所に書き直していました。ここに置きます。

X まわり(窓を引く・倍率・押す・撮る・片づける)は `ribbon_sweep.App` の
物をそのまま借ります。違うのは**起こす相手**と**ソケットの名前**だけです。

    from tools import office_shot            # 使うとき
    a = office_shot.Office(出し先, path="台帳.sheet.adoc")
    箱 = a.boxes()                            # リボンのボタンの箱(id → 箱)
    a.press("freeze")                         # id で押す
    print(a.state()["status"])                # 状態行を読む
    a.shot("freeze-after")                    # 撮る
    a.close()

単体で走らせると、起こして1枚撮って終わります。

    python3 tools/office_shot.py [出し先]

## この道具で踏んだ跡(同じ所で止まらないように)

* **`WAYLAND_DISPLAY` を外さないと落ちます。** gpui が Wayland を掴んで
  `NoCompositor` で panic します。X の画面を使うので必ず外します
* **ファイル選択の窓は開きません。** HOME と `XDG_RUNTIME_DIR` を偽物に
  差し替えているので、rfd が使う DBus のポータルに届きません。窓が1つも
  増えないことを X の一覧で確かめました。開く・保存を試すときは rpc の
  `open` / `save` に径路を直に渡してください
* **撮った絵の y は、窓の中の y より 16 少なくなります**(窓の飾りのぶん)。
  絵から座標を測って押すときは 16 足してください
* **一覧の座標(`ui_state` の `pick`)は格子の面の中の座標です。**
  窓の座標に直すには `ribbon` の `pane` の左上を足します
* **Esc はスライサーごと閉じます。** 一覧だけ閉じたいときは、空のセルを
  押してください
* 前の回の `officework` が `:2` に残っていると窓を取り違えます。
  `Office` は自分の PID の窓しか見ないので取り違えませんが、絵を撮る前に
  `pkill -f target/release/officework` で掃除しておくと確実です
"""

import json
import os
import socket
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ribbon_sweep as rs  # noqa: E402

from Xlib import X, display  # noqa: E402
from Xlib.ext import xtest  # noqa: E402

OFFICE = os.path.join(rs.ROOT, "target", "release", "officework")


class Office:
    """起こして、押して、聞いて、撮る。片づけまで持ちます。"""

    def __init__(self, shots, path=None, home_files=None, lang=None, ready=40):
        self.shots = shots
        os.makedirs(shots, exist_ok=True)
        self.run_dir = tempfile.mkdtemp(prefix="office-shot-")
        env = dict(os.environ)
        # **Wayland を掴ませない。** 掴むと NoCompositor で落ちます
        env.pop("WAYLAND_DISPLAY", None)
        env["XDG_RUNTIME_DIR"] = self.run_dir
        # **HOME を分ける** — 発注者の settings.toml も控えも書き換えない
        self.home = os.path.join(self.run_dir, "home")
        os.makedirs(os.path.join(self.home, ".config", "officework"), exist_ok=True)
        for rel, body in (home_files or {}).items():
            to = os.path.join(self.home, rel)
            os.makedirs(os.path.dirname(to), exist_ok=True)
            with open(to, "w", encoding="utf-8") as f:
                f.write(body)
        env["HOME"] = self.home
        env.setdefault("DISPLAY", ":0")
        # **IME を外す** — 通さないと XTEST の字がかな配列で化けます
        for k in ("GTK_IM_MODULE", "QT_IM_MODULE"):
            env.pop(k, None)
        env["XMODIFIERS"] = "@im=none"
        if lang:
            env["OFFICE_LANG"] = lang
        self.env = env
        self.sock = os.path.join(self.run_dir, "officework", "officework.sock")
        self.log = open(os.path.join(self.run_dir, "office.log"), "w+")
        self.proc = subprocess.Popen(
            [OFFICE] + ([path] if path else []), env=env, stdout=self.log, stderr=self.log
        )
        self.d = display.Display(env["DISPLAY"])
        self.pid_atom = self.d.intern_atom("_NET_WM_PID")
        self._wait_ready(ready)
        self._settle()

    # ---- 起こす -----------------------------------------------------------

    def _wait_ready(self, secs):
        limit = time.time() + secs
        while time.time() < limit:
            if os.path.exists(self.sock):
                try:
                    if self.rpc({"cmd": "ping"}).get("ok"):
                        return
                except Exception:
                    pass
            if self.proc.poll() is not None:
                self.log.seek(0)
                raise SystemExit("officework が落ちました:\n" + self.log.read()[-2000:])
            time.sleep(0.4)
        raise SystemExit(f"{secs} 秒たっても ping に答えません({self.sock})")

    def _settle(self):
        """**窓が育ちきるまで待つ。** 途中の姿で測ると倍率を誤り、以後
        ずっと別の場所を押します(writer_shot が 2026-08-17 に踏んだ跡)。
        """
        前 = None
        for _ in range(40):
            w = self.window()
            now = (w[3], w[4]) if w else None
            if now and now == 前:
                break
            前 = now
            time.sleep(0.25)
        time.sleep(0.6)

    # ---- X まわりは ribbon_sweep のものを借りる ---------------------------

    window = rs.App.window
    scale = rs.App.scale
    click = rs.App.click
    take_focus = rs.App.take_focus
    has_focus = rs.App.has_focus
    key = rs.App.key
    shot = rs.App.shot
    close = rs.App.close

    # ---- 話す -------------------------------------------------------------

    def rpc(self, obj):
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(60)
        s.connect(self.sock)
        s.sendall((json.dumps(obj, ensure_ascii=False) + "\n").encode("utf-8"))
        buf = b""
        while not buf.endswith(b"\n"):
            c = s.recv(65536)
            if not c:
                break
            buf += c
        s.close()
        return json.loads(buf)

    def state(self):
        """いま何が開いているか・状態行・一覧の位置。"""
        return self.rpc({"cmd": "ui_state"})

    def showing(self):
        """いま前に出ているのは表(`sheet`)か文書(`doc`)か。"""
        return self.rpc({"cmd": "ping"}).get("showing")

    def boxes(self, tries=10):
        """リボンのボタンの箱(id → {x, y, w, h})。

        **控えは描いた後に埋まります。** アプリは描く前には答えられないので、
        マウスを少し動かして描き直させながら何度か聞きます。

        **文書の側が前に出ていると空になります。** rpc は表の側の口なので、
        文書のリボンの箱は答えられません(文書は `OFFICEWORK_UI_DUMP` の
        道で、`writer_shot.py` の持ち場です)。黙って空を返すと「ボタンが
        無い」と読み違えるので、そのときは言って止めます。
        """
        if self.showing() == "doc":
            raise SystemExit(
                "文書の側が前に出ています。表のリボンの箱は rpc では取れません。\n"
                "  表を開いてください: Office(出し先, path='….sheet.adoc')\n"
                "  文書を撮るなら tools/writer_shot.py を使ってください"
            )
        for k in range(tries):
            w = self.window()
            if w:
                xtest.fake_input(
                    self.d, X.MotionNotify, x=w[1] + 300 + k * 8, y=w[2] + 500
                )
                self.d.sync()
            time.sleep(0.3)
            r = self.rpc({"cmd": "ribbon"})
            if r.get("boxes"):
                return {b["id"]: b for b in r["boxes"]}
        return {}

    def pane(self):
        """格子の面の左上と大きさ(窓の中の論理座標)。"""
        return self.rpc({"cmd": "ribbon"}).get("pane", [0, 0, 0, 0])

    # ---- 押す -------------------------------------------------------------

    def press(self, btn_id, wait=1.2, tabs=True):
        """リボンのボタンを **id で** 押す。見つからなければ段を順に探します。

        `tabs=False` なら今の段だけを見ます(段を替えたくないとき)。
        """
        b = self.boxes()
        if btn_id not in b and tabs:
            for k in sorted(x for x in b if x.startswith("@tab")):
                self.press_box(b[k], wait=0.7)
                b2 = self.boxes()
                if btn_id in b2:
                    b = b2
                    break
                b = {**b, **{x: y for x, y in b2.items() if x.startswith("@tab")}}
        if btn_id not in b:
            raise SystemExit(
                f"ボタン {btn_id!r} が見つかりません。今の段にあるのは "
                + ", ".join(sorted(x for x in b if not x.startswith("@")))[:200]
            )
        self.press_box(b[btn_id], wait)
        return b[btn_id]

    def press_box(self, box, wait=1.2):
        """箱の真ん中を押す。**押す前に近くへ寄せます** — 遠くから一足飛びに
        押すと、通り道のボタンの状態が変わることがあります(踏み跡)。
        """
        cx, cy = box["x"] + box["w"] / 2, box["y"] + box["h"] / 2
        w = self.window()
        s = self.scale()
        xtest.fake_input(
            self.d, X.MotionNotify, x=w[1] + int((cx - 20) * s), y=w[2] + int(cy * s)
        )
        self.d.sync()
        time.sleep(0.2)
        self.click(cx, cy, wait=wait)

    def pick_click(self, dx, dy, wait=1.2):
        """開いている一覧の項目を押す。座標は**一覧の左上からのずれ**。

        一覧の位置(`ui_state` の `pick`)は格子の面の中の座標なので、
        窓の座標に直すには `pane` の左上を足します。ここがずれていて、
        2026-08-21 に3回外しました。
        """
        pk = self.state().get("pick") or {}
        pane = self.pane()
        self.click(int(pane[0]) + pk.get("x", 0) + dx, int(pane[1]) + pk.get("y", 0) + dy, wait)

    def type(self, text, wait=0.05):
        """ASCII を打つ。**日本語は打てません**(IME を外してあるため)。
        日本語を入れるときは rpc の `set` を使ってください。
        """
        for ch in text:
            code = self.d.keysym_to_keycode(ord(ch))
            up = ch.isupper()
            if up:
                xtest.fake_input(self.d, X.KeyPress, self.d.keysym_to_keycode(0xFFE1))
            xtest.fake_input(self.d, X.KeyPress, code)
            self.d.sync()
            time.sleep(0.03)
            xtest.fake_input(self.d, X.KeyRelease, code)
            if up:
                xtest.fake_input(self.d, X.KeyRelease, self.d.keysym_to_keycode(0xFFE1))
            self.d.sync()
            time.sleep(wait)


if __name__ == "__main__":
    出 = sys.argv[1] if len(sys.argv) > 1 else "/tmp/office-shot"
    a = Office(出)
    try:
        w = a.window()
        s = a.scale()
        print("窓(論理):", round(w[3] / s), round(w[4] / s), " 倍率:", s)
        b = a.boxes()
        print("ボタンの箱:", len(b), "個")
        print("状態行:", a.state().get("status", "")[:60])
        print(a.shot("office"))
    finally:
        a.close()
