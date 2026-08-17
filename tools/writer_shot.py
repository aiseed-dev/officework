#!/usr/bin/env python3
"""writer を実機で起こして、押して、撮る小さな道具。

**writer には calc のような rpc の口が無い**(ソケットは calc だけ)。
なので状態は聞けず、見るのは撮った絵だけ。ribbon_sweep の X まわりの
作りだけを借りる。

    python3 tools/writer_shot.py                 # 起こして1枚撮る

窓の位置・倍率の求め方、撮る前に前面へ出すこと、押す前に近くへ寄せる
ことは ribbon_sweep と同じ踏み跡(2026-08-15)。

## この道具で試せないこと(2026-08-17 に確かめた)

- **保存や書き出しの窓(ファイル選択)は開きません。** HOME と
  XDG_RUNTIME_DIR を偽物に差し替えているので、rfd が使う DBus の
  ポータルに届きません。窓が1つも増えないことを X の一覧で確かめました。
  ファイルを書く所は Rust の試験で見てください(パスを直に渡せます)
- **ファイルの面(タブ0)の項目は控えに出ません。** リボンのボタンと同じ
  仕組みで場所を控えているつもりですが、`ui.json` には出てきません。
  座標で押してください(絵から測る)
"""

import json
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
    def __init__(self, shots, path=None, home_files=None):
        """`home_files` は偽の HOME に**起こす前に**置くファイル。

        `{"…/templates/社内標準.toml": "中身"}` のように、HOME からの相対の
        径路で渡します。配られたテンプレートのように、**アプリが起動時に
        読む物**を試すために要ります(2026-08-18)。
        """
        self.shots = shots
        os.makedirs(shots, exist_ok=True)
        self.run_dir = tempfile.mkdtemp(prefix="writer-shot-")
        env = dict(os.environ)
        env.pop("WAYLAND_DISPLAY", None)
        env["XDG_RUNTIME_DIR"] = self.run_dir
        # **HOME を分ける** — 発注者の settings.toml を書き換えない。
        # 置き場の名前は officework(2026-08-16 に office から移した。
        # ここだけ古い名前のままで、置き場の物は1つも読まれていなかった)
        self.home = os.path.join(self.run_dir, "home")
        os.makedirs(os.path.join(self.home, ".config", "officework"), exist_ok=True)
        for rel, body in (home_files or {}).items():
            to = os.path.join(self.home, rel)
            os.makedirs(os.path.dirname(to), exist_ok=True)
            with open(to, "w", encoding="utf-8") as f:
                f.write(body)
        env["HOME"] = self.home
        env.setdefault("DISPLAY", ":0")
        # **IME を外す** — 通さないと XTEST の字がかな配列で化ける
        for k in ("GTK_IM_MODULE", "QT_IM_MODULE"):
            env.pop(k, None)
        env["XMODIFIERS"] = "@im=none"
        # **リボンの場所を writer に書き出させる**(2026-08-16)。
        # calc の rpc `{"cmd":"ribbon"}` に当たるもの。無いと座標を目分量で
        # 当てることになり、3回外して発注者の打鍵まで拾った
        self.ui_json = os.path.join(self.run_dir, "ui.json")
        env["OFFICEWORK_UI_DUMP"] = self.ui_json
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
        # **窓が育ちきるまで待つ。** 途中の姿(900×1000)で測ると倍率を
        # 1.0 と誤り、以後ずっと**半分の位置**を押す(2026-08-17 に踏んだ。
        # ファイルの面も段も全部効かず、canvas を疑って回り道をした)。
        # 大きさが2回続けて同じになったら落ち着いたと見る
        前 = None
        for _ in range(40):
            w = self.window()
            now = (w[3], w[4]) if w else None
            if now and now == 前:
                break
            前 = now
            time.sleep(0.25)
        time.sleep(0.8)

    window = rs.App.window
    take_focus = rs.App.take_focus
    has_focus = rs.App.has_focus
    shot = rs.App.shot
    close = rs.App.close

    def scale(self):
        """画面の倍率。**writer が書き出した論理の幅と、窓の物理の幅の比**
        (2026-08-17)。

        前は「窓が 1400 より広ければ 2.0」と当てていて、**900×1000 の窓で
        1.0 と誤り、半分の位置を押していた** — ファイルの面の項目が全部
        効かず、canvas を疑って回り道をした。当て推量をやめる。
        """
        w = self.window()
        if not w:
            return 1.0
        try:
            u = self.ui(want_boxes=False)
            lw = float(u.get("win_w") or 0)
            if lw > 0:
                s = w[3] / lw
                if 0.5 <= s <= 4.0:
                    return s
        except Exception:
            pass
        return 2.0 if w[3] > 1400 else 1.0

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

    def ui(self, tries=20, want_boxes=True):
        """いまの画面の様子(段・ボタンの箱・状態行)。**writer が描いた
        ものを読む** — 目分量で座標を当てない。

        **控えは描いた後に埋まる**(canvas の prepaint は render の後)ので、
        読む前にマウスを動かして1フレーム描かせる。踏み跡「押した直後の1手は
        画に出ない」と同じ理由(2026-08-16)。
        """
        for i in range(tries):
            self._nudge()
            time.sleep(0.2)
            try:
                with open(self.ui_json, encoding="utf-8") as f:
                    u = json.load(f)
            except Exception:
                continue
            if not want_boxes or u.get("boxes") or i >= 3:
                return u
        raise SystemExit(f"ui.json が出ません({self.ui_json})。writer が描いていない?")

    def _nudge(self):
        """描き直させるためにマウスを少し動かす(押さない)。

        **段の行の上も通る。** 控えは描いた後に埋まり、writer は描く前に
        控えを書き出すので、読めるのは常に1フレーム前の分です。ファイルの面
        では本文の上でマウスを動かしても描き直しが起きないため、控えが古いまま
        で、面の項目(`f-html` など)が出てきませんでした(2026-08-17)。
        段の行はマウスを乗せると色が変わる = 必ず描き直します。
        """
        w = self.window()
        if not w:
            return
        for x, y in ((300, 190), (300, 700), (320, 700)):
            xtest.fake_input(self.d, X.MotionNotify, x=w[1] + x, y=w[2] + y)
            self.d.sync()
            time.sleep(0.05)

    def tab(self, i, wait=0.9):
        """段を開く(番号)。**段の箱も writer が控えている**ので当てない"""
        for _ in range(4):
            u = self.ui()
            if u["tab"] == i:
                return u
            b = next((x for x in u["boxes"] if x["id"] == f"@tab{i}"), None)
            if b is None:
                raise SystemExit(f"段 {i} の箱がありません")
            self.click(b["x"] + b["w"] / 2, b["y"] + b["h"] / 2, wait)
        raise SystemExit(f"段 {i} に切り替わりません")

    def drag(self, x1, y1, x2, y2, wait=0.8):
        """本文を**マウスで引いて選ぶ**(押す → 動かす → 離す)。

        合成のダブルクリックは語を掴めなかった(2026-08-16)。人が字を
        選ぶ本筋はこちらなので、こちらを持つ。座標は論理。
        """
        w = self.window()
        if not w:
            raise SystemExit("窓が消えました")
        s = self.scale()
        pts = [(x1, y1)] + [
            (x1 + (x2 - x1) * k / 6.0, y1 + (y2 - y1) * k / 6.0) for k in range(1, 7)
        ]
        px, py = pts[0]
        xtest.fake_input(self.d, X.MotionNotify, x=w[1] + int(px * s), y=w[2] + int(py * s))
        self.d.sync()
        time.sleep(0.2)
        xtest.fake_input(self.d, X.ButtonPress, 1)
        self.d.sync()
        for px, py in pts[1:]:
            time.sleep(0.05)
            xtest.fake_input(self.d, X.MotionNotify, x=w[1] + int(px * s), y=w[2] + int(py * s))
            self.d.sync()
        time.sleep(0.1)
        xtest.fake_input(self.d, X.ButtonRelease, 1)
        self.d.sync()
        time.sleep(wait)
        return self.ui()

    def press(self, cmd_id, wait=1.0):
        """リボンのボタンを **id で** 押す(いまの段に見えている物)"""
        u = self.ui()
        b = next((x for x in u["boxes"] if x["id"] == cmd_id), None)
        if b is None:
            raise SystemExit(f"「{cmd_id}」がいまの段に見えません(段 {u['tab']})")
        self.click(b["x"] + b["w"] / 2, b["y"] + b["h"] / 2, wait)
        return self.ui()

    def take(self, name):
        """**撮る前に前面へ出し、マウスを動かして描き直させる。**
        前面に出すだけでは足りない — 押した直後の1手は画に出ないことがある
        (踏み跡 2026-08-15。2026-08-16 に右パネルの面でまた踏んだ)"""
        self.take_focus()
        time.sleep(0.3)
        self._nudge()
        time.sleep(0.4)
        return self.shot(name)

    def hover(self, cid, wait=0.8):
        """ボタンの上にマウスを置いて、**動かさずに**撮れる状態にする。

        `take()` は撮る前にマウスを動かすので、説明(ツールチップ)は
        必ず消えます。説明を見たいときはこれを使い、`shot()` で撮ります
        (2026-08-17、writer のリボンに説明を足したときに気づきました)。
        """
        from Xlib import X
        from Xlib.ext import xtest

        self.take_focus()
        time.sleep(0.3)
        b = next(x for x in self.ui()["boxes"] if x["id"] == cid)
        s = self.scale()
        w = self.window()
        cx, cy = b["x"] + b["w"] / 2, b["y"] + b["h"] / 2
        # **少しずつ寄せる**(いきなり飛ぶと動きとして拾われない — click と同じ)
        for dx, dy in ((20, 20), (8, 8), (0, 0)):
            xtest.fake_input(self.d, X.MotionNotify,
                             x=w[1] + int((cx + dx) * s), y=w[2] + int((cy + dy) * s))
            self.d.sync()
            time.sleep(0.15)
        time.sleep(wait)

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
