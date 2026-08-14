# -*- coding: utf-8 -*-
"""アプリ(calc / writer)を起こす口。

    officework-calc              表計算を開く
    officework-calc 台帳.xlsx    そのファイルを開く
    officework-writer 報告.docx  文書を開く
    officework                   何ができるかを見る
    officework --install-desktop アプリの一覧に出す(Linux)

**pip で配るのが主な道**(2026-08-15 発注者)。署名も公証も要らない —
mac の Gatekeeper も Windows の SmartScreen も「ブラウザで落とした物」に
付く印が引き金で、pip が入れた物には付かないため。Python 同梱も要らない
(pip で入れる人は Python を持っている)。

実行ファイルは wheel の中(`officework/bin/`)に入っている。手元の木で
開発しているときは `target/release/` からも探す。
"""

import os
import subprocess
import sys

APPS = ("calc", "writer")


def _exe(app):
    """アプリの実行ファイルを探す。wheel の中 → 環境変数 → 手元の木 → PATH"""
    import shutil

    name = app + (".exe" if sys.platform == "win32" else "")
    here = os.path.dirname(os.path.abspath(__file__))
    # (1) wheel に同梱した物
    p = os.path.join(here, "bin", name)
    if os.path.exists(p):
        return p
    # (2) 名指し(開発中・別の場所に置いた場合)
    env = os.environ.get("OFFICEWORK_" + app.upper())
    if env and os.path.exists(env):
        return env
    # (3) 手元の木(officework をソースから触っているとき)
    for up in range(2, 6):
        root = os.path.abspath(os.path.join(here, *([".."] * up)))
        p = os.path.join(root, "target", "release", name)
        if os.path.exists(p):
            return p
    # (4) PATH
    return shutil.which(name) or shutil.which("officework-" + app)


def _run(app, argv):
    exe = _exe(app)
    if not exe:
        sys.exit(
            "{} の実行ファイルが見つかりません。\n"
            "この wheel には入っていない形かもしれません — "
            "OFFICEWORK_{} に径路を入れてください".format(app, app.upper())
        )
    # **待たない。** アプリは窓を開けて動き続けるので、端末は返す
    if sys.platform == "win32":
        subprocess.Popen([exe] + argv)
    else:
        subprocess.Popen([exe] + argv, start_new_session=True)
    return 0


def calc():
    return _run("calc", sys.argv[1:])


def writer():
    return _run("writer", sys.argv[1:])


def install_desktop():
    """アプリの一覧に出す(Linux の .desktop を置く)。

    pip で入れると、実行ファイルはあるがアプリの一覧には出ない。
    これを一度走らせると出る(消すときは --uninstall-desktop)。
    """
    if sys.platform != "linux":
        print("この口は Linux 用です(mac と Windows では要りません)")
        return 1
    share = os.path.expanduser("~/.local/share/applications")
    os.makedirs(share, exist_ok=True)
    made = []
    for app in APPS:
        exe = _exe(app)
        if not exe:
            continue
        名 = "表計算(xlsx)" if app == "calc" else "文書(docx)"
        mime = (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            if app == "calc"
            else "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        )
        cat = "Spreadsheet" if app == "calc" else "WordProcessor"
        path = os.path.join(share, "officework-{}.desktop".format(app))
        with open(path, "w", encoding="utf-8") as f:
            f.write(
                "[Desktop Entry]\n"
                "Type=Application\n"
                "Name=officework {}\n"
                "Comment={}\n"
                "Exec={} %f\n"
                "Terminal=false\n"
                "Categories=Office;{};\n"
                "MimeType={};\n".format(app, 名, exe, cat, mime)
            )
        made.append(path)
    if not made:
        sys.exit("実行ファイルが見つかりません")
    subprocess.run(["update-desktop-database", share], capture_output=True)
    print("アプリの一覧に出しました:")
    for p in made:
        print("  ", p)
    return 0


def uninstall_desktop():
    share = os.path.expanduser("~/.local/share/applications")
    n = 0
    for app in APPS:
        p = os.path.join(share, "officework-{}.desktop".format(app))
        if os.path.exists(p):
            os.remove(p)
            n += 1
    print("{} 件を消しました".format(n))
    return 0


def main():
    """`officework` を素で打ったとき。何ができるかを見せる"""
    args = sys.argv[1:]
    if args and args[0] == "--install-desktop":
        return install_desktop()
    if args and args[0] == "--uninstall-desktop":
        return uninstall_desktop()
    if args and args[0] in APPS:
        return _run(args[0], args[1:])

    from . import __doc__ as pkg_doc  # noqa: F401

    print("officework — 表計算と文書。Python でマクロが書けます。\n")
    print("  officework-calc [ファイル]      表計算を開く")
    print("  officework-writer [ファイル]    文書を開く")
    print("  officework --install-desktop    アプリの一覧に出す(Linux)")
    print()
    print("Python から使う(こちらが主):")
    print("    from officework import calc as xw")
    print("    wb = xw.Book()          # calc が起ち上がる")
    print('    wb.sheets[0]["A1"].value = "こんにちは"')
    print()
    print("ファイルだけ触る(アプリは要りません):")
    print("    from officework import sheet, doc")
    print()
    for app in APPS:
        exe = _exe(app)
        print("  {}: {}".format(app, exe or "見つかりません"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
