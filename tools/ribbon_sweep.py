#!/usr/bin/env python3
"""リボンの全ボタンを実機で一巡して点検する。

**画面は見ないと分からない。** 2026-08-08、一覧の位置を直したつもりで
「リボンを押すと格子から焦点が外れ、Esc も他のキーも一切効かなくなる」
不具合を実機ではじめて見つけた。40 個の一覧が開くボタン全部に効いていた
のに、単体試験も台帳も何も言わなかった。そこでこの道具を置く。

やること: 段(タブ)ごとに押せるボタンを順に押し、押すたびに calc.sock
から画面の状態を聞いて、下の4点を確かめる。

1. 落ちない
2. 押して**何かが起きる**(一覧・パネル・状態行・中身のどれかが変わる)
3. 一覧が開いたら、**押したボタンの真下**に出ている(横のずれが小さい)
4. Esc で閉じ、**閉じたあとキーが効く**(= 焦点が格子に戻っている)

判定は画素比べでなく rpc の `ribbon` / `ui_state` を使う。撮るのは
しくじった時だけ(scratch/ 以下に置く)。

使い方:

    python3 tools/ribbon_sweep.py                 # ぜんぶの段
    python3 tools/ribbon_sweep.py --tabs 1 2      # 段を選ぶ
    python3 tools/ribbon_sweep.py --keep          # 終わっても閉じない

前提: X11(この機械は GNOME Wayland なので XWayland 経由)、python-xlib、
ImageMagick の import。Xephyr は DRI3 が無く GPUI が真っ黒になるので使わない。
"""

import argparse
import json
import os
import re
import socket
import subprocess
import sys
import tempfile
import time

from Xlib import X, XK, Xatom, display
from Xlib.ext import xtest

# 「押して何か起きたか」「Esc で閉じるか」を見るか。**既定は見ない。**
# この2つは誤報が多く(手では効く zoom-in を「無反応」と言う、長く回すと
# Esc の判定が崩れる)、当たらない検査は無いより悪いので --strict に隔離
# した。数で見る「位置」の検査だけは当てになる — 実際に6箇所見つけた
STRICT = False
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# 点検で立てた calc の pid を書き留める場所(後始末はここの分だけ殺す)
PIDFILE = os.path.join(tempfile.gettempdir(), "jo-sweep-pids")
# **押さないボタン。** 機械の「ファイルを選ぶ小窓」が出て点検が止まる。
# 黙って飛ばさず、終わりに何を飛ばしたか必ず並べる
SKIP = {
    "open": "ファイルを選ぶ小窓",
    "insimage": "ファイルを選ぶ小窓",
    "data-from-text": "ファイルを選ぶ小窓",
    "data-external-links": "ファイルを選ぶ小窓",
    "saveas": "ファイルを選ぶ小窓",
    "pdf": "ファイルを選ぶ小窓",
    "quit": "終了",
}
CALC = os.path.join(ROOT, "target", "release", "calc")
# 一覧の左端がボタンの左端からこれ以上ずれていたら「真下でない」
X_SLACK = 4.0


class App:
    """試験用に立てた calc 一つ。窓と socket の世話をする"""

    def __init__(self, shots):
        self.shots = shots
        # **実行時ディレクトリを分ける。** 分けないと、動いている本人の
        # calc に相乗りして自分の窓が出ない(単独起動の仕組みがあるため)
        self.run_dir = tempfile.mkdtemp(prefix="ribbon-sweep-")
        env = dict(os.environ)
        env.pop("WAYLAND_DISPLAY", None)  # 消すと gpui は X11 を選ぶ
        env["XDG_RUNTIME_DIR"] = self.run_dir
        # **HOME も分ける。** 設定は `$HOME/.config/office/settings.toml` に
        # あり、writer と calc と**発注者の窓**で1つを共有している。
        # 点検で押したボタン(暗い画面・文字の大きさ)がそこへ書き込まれ、
        # 2026-08-10 に発注者の ui_scale を 1.5 に変えてしまった。
        # 直近ファイル・復旧の控え・鍵もここに集まるので、まとめて外へ出す
        self.home = os.path.join(self.run_dir, "home")
        os.makedirs(os.path.join(self.home, ".config", "office"), exist_ok=True)
        env["HOME"] = self.home
        env.setdefault("DISPLAY", ":2")
        self.env = env
        self.log = open(os.path.join(self.run_dir, "calc.log"), "w+")
        self.proc = subprocess.Popen([CALC], env=env, stdout=self.log, stderr=self.log)
        # **自分が立てた calc の pid だけを控える。** 後始末に
        # `pkill -f release/calc` を使うと、発注者が開いている窓まで
        # 巻き添えにする(2026-08-09 に気づいた。実際に何度もやっていた)
        with open(PIDFILE, "a") as f:
            f.write(f"{self.proc.pid}\n")
        # **必ず自分の実行時ディレクトリの socket を使う。** 共有の
        # /tmp/officework-<uid>/ に落とすと、発注者が動かしている calc に
        # 話しかけてしまう(一度やった)。まだ無い間は待つ
        self.sock_path = os.path.join(self.run_dir, "officework", "calc.sock")
        self.d = display.Display(env["DISPLAY"])
        self.pid_atom = self.d.intern_atom("_NET_WM_PID")
        self._wait_ready()

    def _wait_ready(self, secs=40):
        end = time.time() + secs
        while time.time() < end:
            if self.proc.poll() is not None:
                raise SystemExit(f"calc が起動しませんでした:\n{self._log_tail()}")
            try:
                if not os.path.exists(self.sock_path):
                    raise FileNotFoundError(self.sock_path)
                if self.rpc({"cmd": "ping"}).get("ok") and self.window():
                    time.sleep(1.5)  # 最初の描画を待つ(場所が控えられる)
                    self.take_focus()
                    return
            except Exception:
                pass
            time.sleep(0.7)
        raise SystemExit(f"calc が応答しませんでした:\n{self._log_tail()}")

    def _log_tail(self):
        self.log.flush()
        self.log.seek(0)
        return "".join(self.log.readlines()[-20:])

    def alive(self):
        return self.proc.poll() is None

    # --- socket ---------------------------------------------------------
    def rpc(self, obj):
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(5)
        s.connect(self.sock_path)
        s.sendall((json.dumps(obj) + "\n").encode())
        buf = b""
        while not buf.endswith(b"\n"):
            chunk = s.recv(65536)
            if not chunk:
                break
            buf += chunk
        s.close()
        return json.loads(buf.decode())

    def state(self):
        """いまの画面の状態。**答えが欠けていたら黙って進まない** —
        様子見の口が壊れたまま点検を続けると、全部が緑に見えてしまう"""
        try:
            r = self.rpc({"cmd": "ui_state"})
        except Exception as e:
            raise SystemExit(f"ui_state が返りません({e})。calc が固まった?")
        if "pick" not in r:
            raise SystemExit(f"ui_state の答えがおかしい: {r}")
        return r

    def reset(self, pane):
        """画面を素の状態へ戻す。**戻せたか必ず確かめる。**
        開きっぱなしのパネルを引きずると、後のボタンが軒並み「無反応」に
        見えて点検が嘘をつく"""
        for _ in range(3):
            st = self.state()
            if not st["open"] and st["pick"] is None:
                return True
            self.key("Escape", 0.4)
        self.click(pane[0] + 400, pane[1] + 300, 0.5)
        self.key("Escape", 0.4)
        st = self.state()
        return not st["open"] and st["pick"] is None

    def restart(self):
        """戻せないときは立て直す(そこまでの点検は残す)。
        **種まきもやり直す** — 空のブックだと書式のボタンが効いても
        差が出ず、後続が軒並み「無反応」に見える"""
        self.close()
        App.__init__(self, self.shots)
        self.seed()

    def seed(self):
        """**見えている所を中身で埋める。** 素の状態に戻すときに押すセルが
        空だと、書式のボタンが効いても差が出ず「無反応」と誤報する"""
        rows = [[f"{r}-{c}" if c % 3 else r * 10 + c for c in range(12)]
                for r in range(1, 41)]
        self.rpc({"cmd": "set", "a1": "A1", "values": rows})

    # --- 窓と入力 -------------------------------------------------------
    def window(self):
        """calc の窓を **PID で** 引く。窓 ID も位置も起動ごとに変わる"""
        found = []

        def walk(w):
            try:
                p = w.get_full_property(self.pid_atom, Xatom.CARDINAL)
                if p and p.value[0] == self.proc.pid:
                    g = w.get_geometry()
                    t = w.translate_coords(self.d.screen().root, 0, 0)
                    found.append((w, -t.x, -t.y, g.width, g.height))
            except Exception:
                pass
            try:
                for c in w.query_tree().children:
                    walk(c)
            except Exception:
                pass

        walk(self.d.screen().root)
        return max(found, key=lambda r: r[3] * r[4]) if found else None

    def click(self, wx, wy, wait=0.9):
        """窓の中の座標を押す"""
        w = self.window()
        if not w:
            raise SystemExit("窓が消えました(落ちた?)")
        xtest.fake_input(self.d, X.MotionNotify, x=w[1] + int(wx), y=w[2] + int(wy))
        self.d.sync()
        time.sleep(0.25)
        xtest.fake_input(self.d, X.ButtonPress, 1)
        self.d.sync()
        time.sleep(0.1)
        xtest.fake_input(self.d, X.ButtonRelease, 1)
        self.d.sync()
        time.sleep(wait)

    def take_focus(self):
        """窓を前面に出して X の入力焦点を取る。**起動のときだけ呼ぶ。**

        X の入力焦点は窓ごと。calc に焦点が無いと Esc は他のアプリへ行き、
        こちらは「Esc が効かない」と誤報する(2026-08-08 それで存在しない
        不具合を発注者に報告した)。前面にも出す — 他の窓が重なっていると
        撮った絵に相手の中身が写る。

        **毎回呼んではいけない。** 入力のたびに持ち上げると、直後の1打鍵が
        取りこぼされて「Esc 一回で閉じない」を 38 件でっち上げる(これも
        2026-08-08 に踏んだ)。
        """
        w = self.window()
        if not w:
            raise SystemExit("窓が消えました(落ちた?)")
        w[0].configure(stack_mode=X.Above)
        self.d.sync()
        self.d.set_input_focus(w[0], X.RevertToParent, X.CurrentTime)
        self.d.sync()
        time.sleep(1.0)

    def has_focus(self):
        """焦点がまだ calc にあるか。**ずれた時だけ**取り直す。

        窓は枠に包まれていることがあるので、calc の窓から**上へ**辿って
        焦点の窓に行き当たるかも見る。`query_tree().parent` は根で int を
        返すので `.id` を取ると例外になる — そこを取りこぼすと常に False に
        なり、打鍵のたびに焦点を取り直して**最初の1打鍵を食う**
        (2026-08-08 それで「Esc 一回で閉じない」を 38 件でっち上げた)。
        """
        w = self.window()
        if not w:
            return False
        try:
            fid = getattr(self.d.get_input_focus().focus, "id", 0)
        except Exception:
            return False
        node = w[0]
        for _ in range(4):
            if getattr(node, "id", None) == fid:
                return True
            try:
                node = node.query_tree().parent
            except Exception:
                break
            if not hasattr(node, "id"):
                break
        return False

    def key(self, name, wait=0.7):
        if not self.has_focus():
            self.take_focus()
        kc = self.d.keysym_to_keycode(XK.string_to_keysym(name))
        xtest.fake_input(self.d, X.KeyPress, kc)
        self.d.sync()
        time.sleep(0.06)
        xtest.fake_input(self.d, X.KeyRelease, kc)
        self.d.sync()
        time.sleep(wait)

    def shot(self, name):
        w = self.window()
        if not w:
            return None
        path = os.path.join(self.shots, f"{name}.png")
        subprocess.run(
            ["import", "-window", hex(w[0].id), path], env=self.env,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        return path

    def close(self):
        try:
            self.proc.terminate()
            self.proc.wait(5)
        except Exception:
            self.proc.kill()

    @staticmethod
    def kill_strays():
        """**点検で立てた分だけ**を落とす。発注者の窓には触らない"""
        try:
            pids = [int(x) for x in open(PIDFILE).read().split()]
        except Exception:
            return 0
        n = 0
        for pid in pids:
            try:
                os.kill(pid, 15)
                n += 1
            except ProcessLookupError:
                pass
            except Exception:
                pass
        open(PIDFILE, "w").close()
        return n


def back_to_tab(app, tab):
    """段を開き直す。**見出しが消えていることがある** — 文脈の段
    (ピボット・表のデザイン)が出入りすると並びが変わるため。
    無ければ黙って諦めて False を返す(例外で点検全体を落とさない)"""
    box = {b["id"]: b for b in app.rpc({"cmd": "ribbon"})["boxes"]}.get(f"@tab{tab}")
    if not box:
        return False
    app.click(box["x"] + box["w"] / 2, box["y"] + box["h"] / 2)
    return app.state()["tab"] == tab


def sweep_tab(app, tab, out, skipped):
    """段を一つ開いて、その段のボタンを順に押す"""
    if not back_to_tab(app, tab):
        # 文脈の段(ピボット・表のデザイン)は普段は出ていない。無いだけ
        skipped.append((f"@tab{tab}", "いま出ていない段"))
        return 0
    r = app.rpc({"cmd": "ribbon"})
    pane_x, pane_y = r["pane"][0], r["pane"][1]
    ids = [b["id"] for b in sorted(r["boxes"], key=lambda b: (b["y"], b["x"]))
           if not b["id"].startswith("@")]
    n = 0
    for bid in ids:
        # **押すたびに位置を引き直す。** 選んでいる物によって出たり消えたり
        # するボタンがあり、段の頭で読んだ座標はすぐ古くなる。古い座標で
        # 押すと隣のボタンを叩き、その結果を無実のボタンのせいにする
        # (2026-08-08 これで「無反応」「居座り」を誤報した)
        if bid in SKIP:
            skipped.append((bid, SKIP[bid]))
            continue
        cur = {b["id"]: b for b in app.rpc({"cmd": "ribbon"})["boxes"]}
        b = cur.get(bid)
        if not b or b["w"] <= 0 or b["h"] <= 0:
            continue
        bx, by, bw, bh = b["x"], b["y"], b["w"], b["h"]
        # 押す前に素の状態へ。戻せなければ立て直してから続ける
        if not app.reset((pane_x, pane_y)):
            out.append((bid, "居座り", "前のパネルが Esc でも格子押しでも閉じない", app.shot(f"izasuwari-{bid}")))
            app.restart()
            if not back_to_tab(app, tab):
                out.append((bid, "段", f"立て直したあと段 {tab} に戻れない", None))
                return n
            r = app.rpc({"cmd": "ribbon"})
            pane_x, pane_y = r["pane"][0], r["pane"][1]
        before = app.state()
        app.click(bx + bw / 2, by + bh / 2)
        n += 1
        if not app.alive():
            out.append((bid, "落ちた", app._log_tail().strip()[-300:], None))
            return n
        after = app.state()

        # 2. 押して何かが起きたか
        changed = (
            after["pick"] != before["pick"]
            or after["open"] != before["open"]
            or after["status"] != before["status"]
            or after["edits"] != before["edits"]
            or after["toggles"] != before["toggles"]
            or after["tab"] != before["tab"]
        )
        if not changed and STRICT:
            out.append((bid, "無反応", "押しても画面も状態行も変わらない", app.shot(f"mu-{bid}")))

        # 3. 一覧が開いたなら押したボタンの真下か
        pick = after["pick"]
        if pick and pick != before["pick"]:
            want = bx - pane_x
            if abs(pick["x"] - want) > X_SLACK and pick["x"] > 0:
                out.append((bid, "位置",
                            f"一覧の左端が {pick['x']:.0f}、ボタンは {want:.0f}",
                            app.shot(f"ichi-{bid}")))

        # 4. Esc で閉じ、閉じたあともキーが効くか
        # 図形・画像を選んだ状態は「開いたパネル」ではない(挿した物が
        # 選ばれたまま残るのは本家と同じ作法)。Esc の対象から外す
        SELECTION = {"shape_sel"}
        opened = set(after["open"]) - set(before["open"]) - SELECTION
        if (pick and pick != before["pick"]) or opened:
            app.key("Escape")
            esc = app.state()
            still = (pick and esc["pick"] == pick) or (set(esc["open"]) & opened)
            if still and STRICT:
                out.append((bid, "Esc",
                            f"Esc 一回で閉じない(pick={esc['pick'] is not None} open={esc['open']})",
                            app.shot(f"esc-{bid}")))
        # 段が変わってしまったら戻す(ファイルの画面など)
        if app.state()["tab"] != tab:
            app.key("Escape")
            if not back_to_tab(app, tab):
                out.append((bid, "段", f"{bid} を押したあと段 {tab} に戻れない", app.shot(f"dan-{bid}")))
                return n
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tabs", nargs="*", type=int, help="段の番号(既定: ファイル以外ぜんぶ)")
    ap.add_argument("--keep", action="store_true", help="終わっても閉じない")
    ap.add_argument("--strict", action="store_true",
                    help="「押して何か起きたか」「Esc で閉じるか」も見る"
                         "(**まだ誤報が多い**。既定は落ちない・位置だけ)")
    ap.add_argument("--shots", default=os.path.join(tempfile.gettempdir(), "ribbon-sweep"))
    a = ap.parse_args()
    global STRICT
    STRICT = a.strict
    os.makedirs(a.shots, exist_ok=True)
    if not os.path.exists(CALC):
        raise SystemExit(f"{CALC} がありません。cargo build --release -p calc を先に")

    app = App(a.shots)
    skipped = []
    # **中身のあるセルを選んでおく。** 空のセルだと書式のボタンを押しても
    # 何も変わらず、効いているのに「無反応」と出てしまう
    app.seed()
    out = []
    total = 0
    try:
        # 段の見出しの場所。ribbon の箱には入っていないので目分量だが、
        # 切り替わったかは rpc の tab で必ず確かめる
        # 「ファイル」(0)は全画面の別物なので既定では回さない
        n_tabs = len([b for b in app.rpc({"cmd": "ribbon"})["boxes"]
                      if b["id"].startswith("@tab")])
        tabs = a.tabs if a.tabs else list(range(1, n_tabs))
        for t in tabs:
            print(f"-- 段 {t} …", flush=True)
            total += sweep_tab(app, t, out, skipped)
    finally:
        if not a.keep:
            app.close()

    print(f"\n押したボタン: {total}")
    if skipped:
        print(f"押さなかったボタン: {len(skipped)} — " +
              "、".join(f"{i}({w})" for i, w in skipped))
    if not out:
        print("しくじりなし。")
        return 0
    print(f"しくじり: {len(out)} 件\n")
    for bid, kind, msg, shot in out:
        print(f"  [{kind}] {bid}: {msg}")
        if shot:
            print(f"          {shot}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
