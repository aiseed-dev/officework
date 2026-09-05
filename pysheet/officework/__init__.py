# -*- coding: utf-8 -*-
"""officework — 帳票を壊さないエンジンと、動いているオフィスソフトを操る橋。

    from officework import sheet      # エンジン: xlsx。アプリは要らない
    from officework import doc        # エンジン: docx。アプリは要らない
    from officework import calc as xw # 橋: 動いている officework を操る

**エンジン**は Rust(pyo3)。原本を正として、変えた所だけ書き戻すので、
罫線・結合・列幅・図形が openpyxl のように壊れない。docx も同じで、
様式・ヘッダー・図形・変更履歴が python-docx のように崩れない。読めなかった物は
`unsupported` に出る(黙って落とさない)。pptx のエンジンも
同じ名前空間に足す予定(officework.slide)。

**橋**は純 Python。ソケット($XDG_RUNTIME_DIR/officework/officework.sock、
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
    # **長さと色は本家と同じ置き場にも出します**(2026-09-01)。
    # 本家の見本は `docx.shared.Pt` と書くので、`officework.shared.Pt`
    # でも通るようにします
    from . import shared  # noqa: F401
    from . import enum  # noqa: F401
    # **文書を作る口は頭にも置きます**(2026-09-01)。本家は
    # `docx.Document()` と書くので、`officework.Document()` でも通します
    from ._doc import Document  # noqa: F401
    # **表計算も本家と同じ置き場に出します**(2026-09-01)。openpyxl は
    # `openpyxl.Workbook()` と `openpyxl.styles` と書きます
    from . import styles  # noqa: F401
    from . import utils  # noqa: F401
    from .sheet import Workbook, load_workbook  # noqa: F401
    _sheet_error = None
except Exception as e:  # pragma: no cover
    # **名前を作らない** — None を入れると from officework import sheet が
    # 黙って None を返し、後で意味不明な AttributeError になる
    _sheet_error = e


def __getattr__(name):
    # sheet も doc も同じ拡張(_sheet.so)の中にいるので、読めない理由は1つ
    if name in ("sheet", "doc", "shared", "enum", "Document",
                "styles", "utils", "Workbook", "load_workbook"):
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


def app_name(単体="calc"):
    """**橋の話し相手の名前。**

    配るのは `officework` の1本です(SEKKEI 段11)。ただし calc と writer の
    単体は開発と試験の道具として残るので、**そちらが動いていればそちらへ
    繋ぎます** — 手元で単体を起こして試している人の邪魔をしないためです。

    どちらも動いていなければ `officework` を返します(起こす相手)。
    """
    if os.name == "nt":
        return "officework"
    for 名 in ("officework", 単体):
        try:
            if _alive(sock_path(名)):
                return 名
        except OfficeworkError:
            break
    return "officework"


def _alive(path):
    """そのソケットに**今つながるか**。ファイルがあるだけでは足りない —
    落ちたアプリのソケットは残るので、存在で見ると死んだ相手を選んで
    「つながりません」になる(2026-09-05、Claude Code の道具で実際に踏んだ)。
    """
    if not os.path.exists(path):
        return False
    try:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(0.5)
        s.connect(path)
        s.close()
        return True
    except OSError:
        return False


def _find_app(app):
    """アプリの実行ファイルを探す。**Python が主なので、こちらが起こす**
    (発注者 2026-08-15「主従が逆転」)。

    探し方は `officework.app` の1本に集める — 2箇所に書くと必ずずれる
    (実際、wheel に同梱していた頃、その実行ファイルをこちらが見て
    いなかった)。
    """
    from .app import _exe

    return _exe(app)


def launch(app, path=None, wait=20.0):
    """アプリを起こして、繋がるまで待つ。既に動いていれば何もしない。

    **openpyxl は画面を持たなかった。** aiseed office は画面があるので、
    Python から呼べば**画面が出て、そこを操れる**(発注者 2026-08-15)。

    画面は**この荷物には入っていません**(2026-08-21)。機械に入っている
    物を探して起こします。無ければ入れ方を言います。
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
        # **入れ方まで言います**(2026-08-21)。画面は別の荷物になったので、
        # 「見つかりません」だけでは、どこから取ればいいか分かりません
        from .app import _HOWTO

        raise OfficeworkError(_HOWTO)
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
