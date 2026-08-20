# -*- coding: utf-8 -*-
"""officework — 帳票を壊さないエンジンと、動いているオフィスソフトを操る橋。

    from officework import sheet      # エンジン: xlsx。アプリは要らない
    from officework import doc        # エンジン: docx。アプリは要らない
    from officework import calc as xw # 橋: 動いている calc を操る

**エンジン**は Rust(pyo3)。原本を正として、変えた所だけ書き戻すので、
罫線・結合・列幅・図形が openpyxl のように壊れない。docx も同じで、
様式・ヘッダー・図形・変更履歴が python-docx のように崩れない。読めなかった物は
`unsupported` に出る(黙って落とさない)。pptx のエンジンも
同じ名前空間に足す予定(officework.slide)。

**橋**は純 Python。アプリごとのソケット($XDG_RUNTIME_DIR/officework/<app>.sock、
径路が AF_UNIX の 108 字上限を超えるときは /tmp/officework-UID/)へ
JSON を1行ずつ。**この機械の中だけ**で、ネットには出ない。

表計算は `from officework import calc as xw`(xlwings 流の Book / Range)。
文書(writer)の橋は今後ここに増える。
"""

import json
import os
import socket

# エンジン(Rust)。橋だけを使うときは無くてよいが、**黙って None にしない** —
# 触ったときに、入っていない理由をそのまま見せる。
# sheet.py / _doc.py は _sheet(Rust)を包んだ純 Python の互換層
# (openpyxl / python-docx の口。台帳: docs/pysheet-gokan.ja.md)
try:
    from . import sheet  # noqa: F401
    from . import _doc as doc  # noqa: F401
    _sheet_error = None
except Exception as e:  # pragma: no cover
    # **名前を作らない** — None を入れると from officework import sheet が
    # 黙って None を返し、後で意味不明な AttributeError になる
    _sheet_error = e


def __getattr__(name):
    # sheet も doc も同じ拡張(_sheet.so)の中にいるので、読めない理由は1つ
    if name in ("sheet", "doc"):
        raise ImportError(
            "officework のエンジン(_sheet)が読めません: {!r}".format(_sheet_error)
        ) from _sheet_error
    raise AttributeError(name)


class OfficeworkError(RuntimeError):
    pass


# 旧名との互換
JoofficeError = OfficeworkError
JocalcError = OfficeworkError


def sock_path(app):
    # **Windows ではアプリはソケットを作らない**(2026-08-20 発注者の決め)。
    # ここで理由を言って断る — 黙って進むと AF_UNIX が無いという
    # 分かりにくいエラーで落ちる
    if os.name == "nt":
        raise OfficeworkError(
            "Windows ではアプリのソケットを作らないので、橋(officework.calc)は"
            "使えません。エンジン(officework.sheet / officework.doc)は"
            "そのまま使えます"
        )
    base = os.environ.get("XDG_RUNTIME_DIR")
    if base:
        p = os.path.join(base, "officework", app + ".sock")
        if len(p.encode()) <= 90:
            return p
    return os.path.join(
        "/tmp", "officework-{}".format(os.getuid()), app + ".sock"
    )


def _find_app(app):
    """アプリの実行ファイルを探す。**Python が主なので、こちらが起こす**
    (発注者 2026-08-15「主従が逆転」)。

    探し方は `officework.app` の1本に集める — 2箇所に書くと必ずずれる
    (実際、wheel に同梱した実行ファイルをこちらが見ていなかった)。
    """
    from .app import _exe

    return _exe(app)


def launch(app, path=None, wait=20.0):
    """アプリを起こして、繋がるまで待つ。既に動いていれば何もしない。

    **openpyxl は画面を持たなかった。** officework は自前の画面があるので、
    Python から呼べば**画面が出て、そこを操れる**(発注者 2026-08-15)。
    """
    import subprocess
    import time

    if os.path.exists(sock_path(app)):
        try:
            call(app, "ping")
            return False          # もう動いている
        except OfficeworkError:
            pass                  # 死んだソケットが残っているだけ
    exe = _find_app(app)
    if not exe:
        raise OfficeworkError(
            "{} の実行ファイルが見つかりません。"
            "OFFICEWORK_{} に径路を入れるか、PATH に置いてください".format(
                app, app.upper()
            )
        )
    args = [exe] + ([os.path.abspath(path)] if path else [])
    subprocess.Popen(
        args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True
    )
    limit = time.time() + wait
    while time.time() < limit:
        try:
            call(app, "ping")
            return True
        except OfficeworkError:
            time.sleep(0.2)
    raise OfficeworkError(
        "{} を起こしましたが、{:.0f} 秒たっても繋がりません".format(app, wait)
    )


def call(app, cmd, **kw):
    req = {"cmd": cmd}
    req.update(kw)
    try:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(10.0)
        s.connect(sock_path(app))
    except OSError as e:
        raise OfficeworkError(
            "{} に繋がりません({}: {})。"
            "officework.launch({!r}) で起こせます".format(
                app, sock_path(app), e, app
            )
        ) from None
    try:
        s.sendall((json.dumps(req, ensure_ascii=False) + "\n").encode("utf-8"))
        buf = b""
        while not buf.endswith(b"\n"):
            chunk = s.recv(65536)
            if not chunk:
                break
            buf += chunk
    finally:
        s.close()
    resp = json.loads(buf.decode("utf-8"))
    if "err" in resp:
        raise OfficeworkError(resp["err"])
    return resp
