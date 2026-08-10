#!/usr/bin/env python3
"""鍵(ショートカット)を実機で押して確かめる。

**「キーの嘘」を落とす試験がこれまで無かった**(sekkei/sugata.ja.md)。
束縛は ui にあり受け口は各アプリ、という作りなので、`KeyBinding` を足して
`on_action` を忘れても、単体試験も wiring_tests も何も言わない。
2026-08-10、7つ足して7つとも受け口を書き忘れたまま「入れた」と言い掛けた。
それを落とすためにこれを置く。

    python3 tools/key_check.py            # calc(rpc で中身を見る)
    python3 tools/key_check.py --writer   # writer(絵を撮る。rpc の口が無い)
    python3 tools/key_check.py --keep     # 終わっても閉じない

ribbon_sweep.py の App をそのまま借りる(窓の世話・焦点・後始末が同じ)。
判定は画素比べでなく rpc(ui_state / get / get_formula / book_info)。

**打ってすぐ聞かない。** rpc は別の糸から答えるので、押した直後に聞くと
まだ前の状態が返る。それで同じ日、効いている鍵を4つ「効かない」と数えた。
状態行が変わるまで待ってから見る。

いま見ているのは 2026-08-10 に足した束(Ctrl+0 / F1 / Ctrl+; / Ctrl+: /
Alt+PageUp / Alt+PageDown / F4 と、スライサーの Alt+S / Alt+C)。鍵を足したらここにも足す。
"""
import os, sys, tempfile, time, zipfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ribbon_sweep  # noqa
from ribbon_sweep import App  # noqa
from Xlib import X, XK  # noqa
from Xlib.ext import xtest  # noqa

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WRITER = os.path.join(ROOT, "target", "release", "writer")


def three_sheet_book():
    """シートを3枚持つブックをその場で作る。**見本を置かない** —
    シート移動を見るためだけの物を repo に増やさない"""
    path = os.path.join(tempfile.mkdtemp(prefix="key-check-"), "三枚.xlsx")
    ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    rns = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    with zipfile.ZipFile(path, "w") as z:
        ov = "".join(
            f'<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType='
            '"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
            for i in (1, 2, 3))
        z.writestr("[Content_Types].xml",
                   '<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.'
                   'openxmlformats.org/package/2006/content-types"><Default Extension="rels"'
                   ' ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
                   '<Default Extension="xml" ContentType="application/xml"/><Override '
                   'PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-'
                   f'officedocument.spreadsheetml.sheet.main+xml"/>{ov}</Types>')
        z.writestr("_rels/.rels",
                   f'<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{rns.replace("officeDocument/2006", "package/2006")}">'
                   f'<Relationship Id="rId1" Type="{rns}/officeDocument" Target="xl/workbook.xml"/></Relationships>')
        sheets = "".join(f'<sheet name="{n}" sheetId="{i}" r:id="rId{i}"/>'
                         for i, n in enumerate(("一", "二", "三"), 1))
        z.writestr("xl/workbook.xml",
                   f'<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="{ns}" '
                   f'xmlns:r="{rns}"><sheets>{sheets}</sheets></workbook>')
        rels = "".join(f'<Relationship Id="rId{i}" Type="{rns}/worksheet" '
                       f'Target="worksheets/sheet{i}.xml"/>' for i in (1, 2, 3))
        z.writestr("xl/_rels/workbook.xml.rels",
                   f'<?xml version="1.0" encoding="UTF-8"?><Relationships '
                   f'xmlns="{rns.replace("officeDocument/2006", "package/2006")}">{rels}</Relationships>')
        for i, t in enumerate(("いち", "にい", "さん"), 1):
            z.writestr(f"xl/worksheets/sheet{i}.xml",
                       f'<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="{ns}">'
                       f'<sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>{t}</t>'
                       '</is></c></row></sheetData></worksheet>')
    return path


def chord(app, mods, name, wait=0.7):
    """修飾キー付きで押す。mods は ["Control_L", "Shift_L", "Alt_L"] など。

    **離すたびに sync して間を空ける。** まとめて離すと修飾が押されたままに
    なることがあり、次の Return が ctrl-enter に化けて何も起きない。それを
    「Ctrl+: が効かない」と読み違え、あげく配列のせいだと結論しかけた
    (2026-08-10)。効いていなかったのは道具のほう。
    """
    if not app.has_focus():
        app.take_focus()
    d = app.d
    mcs = [d.keysym_to_keycode(XK.string_to_keysym(m)) for m in mods]
    kc = d.keysym_to_keycode(XK.string_to_keysym(name))
    for m in mcs:
        xtest.fake_input(d, X.KeyPress, m)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.KeyPress, kc)
    d.sync()
    time.sleep(0.06)
    xtest.fake_input(d, X.KeyRelease, kc)
    d.sync()
    time.sleep(0.03)
    for m in reversed(mcs):
        xtest.fake_input(d, X.KeyRelease, m)
        d.sync()
        time.sleep(0.03)
    time.sleep(wait)


def chord_until(app, mods, key, tries=3):
    """状態行が変わるまで最大 `tries` 回押す。変わった後の状態を返す。
    `mods` が空なら修飾なしの1打鍵。

    **打鍵の取りこぼしを鍵のせいにしないため。** XTEST の打鍵はときどき
    落ちる(修飾2つで6回に1回ほど、修飾なしでも稀に)。死んだ束縛なら
    3回とも落ちるので、ここで隠れるのは道具の取りこぼしだけ
    """
    before = app.state()["status"]
    for _ in range(tries):
        if mods:
            chord(app, mods, key)
        else:
            app.key(key, 0.5)
        st = settle(app, before, 2.0)
        if st["status"] != before:
            return st
    return app.state()


def settle(app, before, secs=4.0):
    """状態行が `before` から変わるまで待って、変わった後の状態を返す"""
    end = time.time() + secs
    st = app.state()
    while time.time() < end and st["status"] == before:
        time.sleep(0.2)
        st = app.state()
    return st


def press(app, cid, times=1):
    """リボンのボタンを id で押す。**段をまたいで探す** —
    boxes はいま出ている段のぶんしか返らない"""
    seen = set()
    for _ in range(24):
        r = app.rpc({"cmd": "ribbon"})
        boxes = {b["id"]: b for b in r["boxes"]}
        if cid in boxes:
            b = boxes[cid]
            for _ in range(times):
                app.click(b["x"] + b["w"] / 2, b["y"] + b["h"] / 2, 0.6)
            return True
        seen.add(r["tab"])
        tabs = sorted((k for k in boxes if k.startswith("@tab")),
                      key=lambda k: int(k[4:]))
        nxt = next((t for t in tabs if int(t[4:]) not in seen), None)
        if nxt is None:
            raise SystemExit(f"リボンに {cid} がありません(見た段 {sorted(seen)})")
        t = boxes[nxt]
        app.click(t["x"] + t["w"] / 2, t["y"] + t["h"] / 2, 0.6)
    raise SystemExit(f"リボンに {cid} が見つかりません")


def cell_xy(app, a1):
    """A1 のセルの窓内座標(概算)。見出しの幅・行の高さは util.rs の定数"""
    col = ord(a1[0]) - ord("A")
    row = int(a1[1:]) - 1
    pane = app.rpc({"cmd": "ribbon"}).get("pane")
    px, py = (pane[0], pane[1]) if pane else (0, 150)
    return (px + 46 + col * 108 + 40, py + 24 + row * 24 + 12)


def goto(app, a1, tries=4):
    """セルを押して、**本当にそこへ行ったか確かめる。**

    座標は概算なので、時々ひとつ隣に当たる。当たらないまま鍵を押すと
    「鍵が効かない」に化ける — 2026-08-10 にこれで3件でっち上げた。
    ui_state の cur を見て、違えば押し直す。
    """
    for _ in range(tries):
        app.key("Escape", 0.3)   # 開きっぱなしの一覧を閉じてから押す
        app.click(*cell_xy(app, a1))
        end = time.time() + 2
        while time.time() < end:
            if app.state().get("cur") == a1:
                return
            time.sleep(0.2)
    raise SystemExit(f"{a1} に行けません(いま {app.state().get('cur')})")


def main():
    bad = []

    def check(name, ok, saw):
        print(("  OK  " if ok else " NG   ") + name + " — " + saw)
        if not ok:
            bad.append(name)

    app = App(shots=False)
    try:
        app.seed()
        time.sleep(1.0)   # 480 セルの描き直しが済むまで待つ

        # **打鍵が届くまで待ってから始める。** 窓が出ていても X の入力焦点は
        # まだ来ていないことがあり、最初の数打鍵が丸ごと落ちる。それを
        # 「鍵が効かない」と数えると、1回の点検で4件でっち上がる
        # (2026-08-10)。F1 が返ってきたら始める
        if "manual" not in chord_until(app, [], "F1", 8)["status"]:
            raise SystemExit("打鍵が calc に届きません(焦点が取れていない)")

        # --- Ctrl+0 ズームを戻す -------------------------------------
        # **先に倍率をずらせない。** リボンの「拡大」は 13 段目にあり、
        # 段を渡り歩くと箱の座標が当てにならない(3回押しても 1.0 のまま
        # だった)。ここで見るのは「鍵が受け口まで届いて 1.0 になる」まで
        # ——「1.0 でない所から戻る」は util の試験ではなく handler が
        # 3行なので目で読める。Ctrl+= は ui_scale(画面全体)で zoom ではない
        st = chord_until(app, ["Control_L"], "0")
        check("Ctrl+0 ズームを戻す(鍵が届く)",
              "100%" in st["status"] and st["toggles"][6] == 1.0,
              f"{st['toggles'][6]} / {st['status'][:24]}")

        # --- F1 手引き ------------------------------------------------
        st = chord_until(app, [], "F1")
        check("F1 手引き", "manual" in st["status"], st["status"][:44])

        # --- Ctrl+; 日付 / Ctrl+: 時刻 --------------------------------
        for a1, mods, key, name, ok_len, sep_at, sep in (
            ("E5", ["Control_L"], "semicolon", "Ctrl+; 日付", 10, 4, "-"),
            ("E6", ["Control_L", "Shift_L"], "semicolon", "Ctrl+: 時刻", 5, 2, ":"),
        ):
            app.rpc({"cmd": "set", "a1": a1, "values": [[""]]})
            goto(app, a1)
            chord_until(app, mods, key)
            app.key("Return", 0.6)
            time.sleep(0.8)   # 確定が届くまで待つ(急ぐと空のまま読む)
            got = str(app.rpc({"cmd": "get", "a1": a1}).get("values", [[""]])[0][0])
            check(f"{name}を値で入れる",
                  len(got) == ok_len and got[sep_at] == sep, repr(got))

        # --- F4 参照の $ を回す ---------------------------------------
        app.rpc({"cmd": "set", "a1": "E8", "values": [["=A1"]]})
        goto(app, "E8")
        chord_until(app, [], "F2")  # 編集に入る(セルの式が編集欄に載る)
        st = chord_until(app, [], "F4")
        app.key("Return", 0.8)
        time.sleep(0.8)
        got = str(app.rpc({"cmd": "get_formula", "a1": "E8"})
                  .get("formulas", [[""]])[0][0])
        check("F4 参照の $ を回す", got == "=$A$1", f"{got!r} / {st['status'][:24]}")
        # --- Alt+S / Alt+C スライサーの複数選択と解除 -----------------
        # **板が開いていないときは「開いていない」と言う**のも仕様のうち
        app.key("Escape", 0.3)
        st = chord_until(app, ["Alt_L"], "s")
        check("Alt+S 板が無いときはそう言う", "開いていません" in st["status"],
              st["status"][:34])

        goto(app, "A2")
        press(app, "insslicer")
        time.sleep(0.8)
        if "slicer" not in app.state()["open"]:
            check("スライサーを開く", False, str(app.state()["open"]))
        else:
            st = chord_until(app, ["Alt_L"], "s")
            check("Alt+S 複数選択に切り替える", "複数選択" in st["status"],
                  st["status"][:24])
            st = chord_until(app, ["Alt_L"], "s")
            check("Alt+S もう一度で単数に戻る", "単数選択" in st["status"],
                  st["status"][:24])
            st = chord_until(app, ["Alt_L"], "c")
            check("Alt+C 絞りを解除する", "解除" in st["status"], st["status"][:24])
    finally:
        if "--keep" not in sys.argv:
            app.close()

    # --- Alt+PageUp / PageDown シート移動 -----------------------------
    # **別の calc を立てる。** open は未保存の変更を断るので、
    # 上の点検で汚したブックのままでは 3 枚のブックを開けない
    app2 = App(shots=False)
    try:
        r = app2.rpc({"cmd": "open", "path": three_sheet_book()})
        if not r.get("ok"):
            check("3枚のブックを開く", False, str(r))
        else:
            time.sleep(1.0)
            bi = app2.rpc({"cmd": "book_info"})
            names, a0 = bi.get("sheets"), bi.get("active")
            chord(app2, ["Alt_L"], "Next")     # PageDown
            time.sleep(0.5)
            a1 = app2.rpc({"cmd": "book_info"}).get("active")
            chord(app2, ["Alt_L"], "Prior")    # PageUp
            time.sleep(0.5)
            a2 = app2.rpc({"cmd": "book_info"}).get("active")
            check("Alt+PageDown 次のシート", a1 != a0, f"{a0} → {a1} / {names}")
            check("Alt+PageUp 前のシート", a2 == a0, f"{a1} → {a2}")
    finally:
        if "--keep" not in sys.argv:
            app2.close()
    print("\n" + ("すべて通りました" if not bad else "落ちた: " + ", ".join(bad)))
    return 1 if bad else 0


def writer_shots(keep):
    """writer には rpc の口が無いので**絵を撮る**。状態行と文字数を目で読む。
    (文字数が 10 増えていれば日付が本当に入っている、という読み方をする)"""
    shots = tempfile.mkdtemp(prefix="key-check-writer-")
    ribbon_sweep.CALC = WRITER

    class W(App):
        def _wait_ready(self, secs=40):
            end = time.time() + secs
            while time.time() < end:
                if self.proc.poll() is not None:
                    raise SystemExit("writer が起動しませんでした:\n" + self._log_tail())
                if self.window():
                    time.sleep(2.0)
                    self.take_focus()
                    return
                time.sleep(0.7)
            raise SystemExit("writer の窓が出ません:\n" + self._log_tail())

    app = W(shots=shots)
    try:
        for name, fn in (
            ("f1", lambda: app.key("F1", 1.0)),
            ("ctrl0", lambda: chord(app, ["Control_L"], "0", 1.0)),
            ("date", lambda: chord(app, ["Control_L"], "semicolon", 1.0)),
            ("time", lambda: chord(app, ["Control_L", "Shift_L"], "semicolon", 1.0)),
        ):
            fn()
            print(name, app.shot("writer-" + name))
    finally:
        if not keep:
            app.close()
    print("\n撮った絵の下端(状態行)を見ること。ここは人が読む")
    return 0


if __name__ == "__main__":
    if "--writer" in sys.argv:
        sys.exit(writer_shots("--keep" in sys.argv))
    sys.exit(main())
