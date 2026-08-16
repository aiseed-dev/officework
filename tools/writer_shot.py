#!/usr/bin/env python3
"""writer を実機で起こして、押して、撮る小さな道具。

**writer には calc のような rpc の口が無い**(ソケットは calc だけ)。
なので状態は聞けず、見るのは撮った絵だけ。ribbon_sweep の X まわりの
作りだけを借りる。

    python3 tools/writer_shot.py                 # 起こして1枚撮る

窓の位置・倍率の求め方、撮る前に前面へ出すこと、押す前に近くへ寄せる
ことは ribbon_sweep と同じ踏み跡(2026-08-15)。
"""

import os
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ribbon_sweep as rs  # noqa: E402
from Xlib import X, XK, Xatom, display  # noqa: E402
from Xlib.ext import xtest  # noqa: E402

WRITER = os.path.join(rs.ROOT, "target", "release", "writer")


class W:
    def __init__(self, shots, path=None):
        self.shots = shots
        os.makedirs(shots, exist_ok=True)
        self.run_dir = tempfile.mkdtemp(prefix="writer-shot-")
        env = dict(os.environ)
        env.pop("WAYLAND_DISPLAY", None)
        env["XDG_RUNTIME_DIR"] = self.run_dir
        # **HOME を分ける** — 発注者の settings.toml を書き換えない
        self.home = os.path.join(self.run_dir, "home")
        os.makedirs(os.path.join(self.home, ".config", "office"), exist_ok=True)
        env["HOME"] = self.home
        env.setdefault("DISPLAY", ":0")
        # **IME を外す** — 通さないと XTEST の字がかな配列で化ける
        for k in ("GTK_IM_MODULE", "QT_IM_MODULE"):
            env.pop(k, None)
        env["XMODIFIERS"] = "@im=none"
        self.env = env
        self.log = open(os.path.join(self.run_dir, "writer.log"), "w+")
        args = [WRITER] + ([path] if path else [])
        self.proc = subprocess.Popen(args, env=env, stdout=self.log, stderr=self.log)
        self.d = display.Display(env["DISPLAY"])
        self.pid_atom = self.d.intern_atom("_NET_WM_PID")
        for _ in range(60):
            if self.window():
                break
            time.sleep(0.5)
        time.sleep(1.5)

    window = rs.App.window
    take_focus = rs.App.take_focus
    has_focus = rs.App.has_focus
    shot = rs.App.shot
    close = rs.App.close

    def scale(self):
        """rpc が無いので**窓の物理幅から推す**(gpui は 1.0 か 2.0)。
        画面の倍率は calc と同じ機械なので、そちらで実測した 2.0 を既定に
        し、窓が小さければ 1.0 に落とす"""
        w = self.window()
        return 2.0 if w and w[3] > 1400 else 1.0

    def click(self, x, y, wait=0.9):
        """**先に近くへ寄せてから**押す(いきなり飛ぶと押下が拾われない)"""
        w = self.window()
        s = self.scale()
        for dx, dy in ((6, 6), (0, 0)):
            xtest.fake_input(self.d, X.MotionNotify,
                             x=w[1] + int((x + dx) * s), y=w[2] + int((y + dy) * s))
            self.d.sync()
            time.sleep(0.25)
        xtest.fake_input(self.d, X.ButtonPress, 1)
        self.d.sync()
        time.sleep(0.12)
        xtest.fake_input(self.d, X.ButtonRelease, 1)
        self.d.sync()
        time.sleep(wait)

    def key(self, name, wait=0.5):
        if not self.has_focus():
            self.take_focus()
        kc = self.d.keysym_to_keycode(XK.string_to_keysym(name))
        xtest.fake_input(self.d, X.KeyPress, kc)
        self.d.sync()
        time.sleep(0.06)
        xtest.fake_input(self.d, X.KeyRelease, kc)
        self.d.sync()
        time.sleep(wait)

    def take(self, name):
        """**撮る前に前面へ出す** — 出さないと古い画が撮れる"""
        self.take_focus()
        time.sleep(0.4)
        return self.shot(name)

    def close(self, keep=False):
        """writer を止めて、**実行時ディレクトリごと片づける**
        (2026-08-16 発注者「/tmp に calc や writer を保存するのはおかしい」)"""
        try:
            self.proc.terminate()
            self.proc.wait(5)
        except Exception:
            self.proc.kill()
        try:
            self.log.close()
        except Exception:
            pass
        if keep:
            print(f"実行時ディレクトリを残しました: {self.run_dir}")
            return
        import shutil

        shutil.rmtree(self.run_dir, ignore_errors=True)


if __name__ == "__main__":
    shots = sys.argv[1] if len(sys.argv) > 1 else "/tmp/writer-shot"
    a = W(shots)
    try:
        w = a.window()
        s = a.scale()
        print("窓(論理):", round(w[3] / s), round(w[4] / s), " 倍率:", s)
        print(a.take("writer"))
    finally:
        a.close()
