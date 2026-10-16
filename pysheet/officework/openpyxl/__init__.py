"""openpyxl と同じ名前を、同じ場所に並べた互換のモジュールです。

openpyxl で書いたプログラムは、import の行を次のように替えるだけで動きます。

    from officework.openpyxl import Workbook, load_workbook
    from officework.openpyxl.styles import Font

1 字も変えずに動かすときは、プログラムより先に `install()` を呼びます。
`sys.modules["openpyxl"]` にこのモジュールを差し込むので、その後の
`import openpyxl` はこちらを読みます。

    import officework.openpyxl
    officework.openpyxl.install()
    import openpyxl               # officework が動く

officework に無い名前はここにもありません。そこで止まったら、その事が
結果です(SEKKEI「決め: 部品へのデータとコードの入り口は 3 つ」2026-10-17)。
"""
import sys as _sys

from ..sheet import Workbook, load_workbook  # noqa: F401
from . import chart, comments, drawing, styles, utils, workbook, worksheet  # noqa: F401

# openpyxl.open は load_workbook の別名
open = load_workbook  # noqa: A001

__all__ = [
    "Workbook", "load_workbook", "open", "install", "uninstall",
    "chart", "comments", "drawing", "styles", "utils", "workbook", "worksheet",
]


def install():
    """この後の `import openpyxl` が officework を読むようにします。

    本家の openpyxl が入っていても、こちらが先に使われます。元に戻すのは
    `uninstall()` です。
    """
    for name, mod in list(_sys.modules.items()):
        if name == __name__ or name.startswith(__name__ + "."):
            _sys.modules["openpyxl" + name[len(__name__):]] = mod


def uninstall():
    """`install()` で差し込んだ分を外します。"""
    for name in list(_sys.modules):
        if (name == "openpyxl" or name.startswith("openpyxl.")) and \
                _sys.modules[name] is _sys.modules.get(__name__ + name[len("openpyxl"):]):
            del _sys.modules[name]
