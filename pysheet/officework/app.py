# -*- coding: utf-8 -*-
"""アプリ(officework)を起こす口。

    officework                   窓を開く
    officework 台帳.xlsx         そのファイルを開く
    officework 報告.docx         同じ窓の別のタブで開く
    officework --install-desktop アプリの一覧に出す(Linux)
    officework --help            何ができるかを見る

**配るのは officework 1本**(2026-08-19 発注者確定。SEKKEI 段11)。
表も文書も同じ窓のタブで開くので、コマンドを2つに分ける理由が
なくなりました。calc と writer の単体は開発と試験の道具として残ります。

**pip で配るのが主な道**(2026-08-15 発注者)。署名も公証も要りません —
mac の Gatekeeper も Windows の SmartScreen も「ブラウザで落とした物」に
付く印が引き金で、pip が入れた物には付かないためです。Python 同梱も
要りません(pip で入れる人は Python を持っています)。

実行ファイルは wheel の中(`officework/bin/`)に入っています。手元の木で
開発しているときは `target/release/` からも探します。
"""

import os
import subprocess
import sys

APP = "officework"

# 開ける物の MIME 型。**受け渡しの2つ(xlsx / docx)と、うちの形**。
# `.adoc` と `.sheet.adoc` には決まった MIME 型が無いので、
# `text/x-asciidoc` を使います(asciidoctor の界隈で通っている名前)
MIMES = (
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "text/x-asciidoc",
)


def _exe(app=APP):
    """実行ファイルを探す。wheel の中 → 環境変数 → 手元の木 → PATH"""
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
    return shutil.which(name)


# **画面が要る物**。wheel には入れられない(pip はシステムの共有ライブラリを
# 入れられない)ので、無ければ入れ方を言う。Debian 系の名前で書き、
# 他の配り物では apt が無いことを断る
_APT = (
    "libxkbcommon0 libxkbcommon-x11-0 libxcb1 libxcb-xkb1 libfontconfig1 "
    "fonts-noto-cjk"
)


def _missing_libs(exe):
    """繋ぎ先で見つからない共有ライブラリの名前(Linux だけ)。

    **黙って落ちるのを防ぐため。** 無い状態で起こすと、端末に ld の
    そっけない1行が出るだけで、何を入れればいいか分からない。
    """
    if not sys.platform.startswith("linux"):
        return []
    try:
        out = subprocess.run(
            ["ldd", exe], capture_output=True, text=True, timeout=10
        ).stdout
    except Exception:
        return []  # ldd が無い機械では黙って先へ(判断材料が無い)
    return [l.split("=>")[0].strip() for l in out.splitlines() if "not found" in l]


def _run(argv):
    exe = _exe()
    if not exe:
        sys.exit(
            "officework の実行ファイルが見つかりません。\n"
            "この wheel には入っていない形かもしれません — "
            "OFFICEWORK_OFFICEWORK に径路を入れてください"
        )
    lack = _missing_libs(exe)
    if lack:
        sys.exit(
            "画面を出すのに要るライブラリがこの機械にありません:\n"
            "  " + "\n  ".join(lack) + "\n\n"
            "pip では入れられないので、機械の側で入れてください。\n"
            "Debian / Ubuntu なら:\n"
            "  sudo apt install " + _APT + "\n"
            "(他の配り物では名前が違います。日本語のフォントも要ります)"
        )
    # **待たない。** アプリは窓を開けて動き続けるので、端末は返す
    if sys.platform == "win32":
        subprocess.Popen([exe] + argv)
    else:
        subprocess.Popen([exe] + argv, start_new_session=True)
    return 0


def install_desktop():
    """アプリの一覧に出す(Linux の .desktop を置く)。

    pip で入れると、実行ファイルはあるがアプリの一覧には出ません。
    これを一度走らせると出ます(消すときは --uninstall-desktop)。
    """
    if sys.platform != "linux":
        print("この口は Linux 用です(mac と Windows では要りません)")
        return 1
    exe = _exe()
    if not exe:
        sys.exit("実行ファイルが見つかりません")
    share = os.path.expanduser("~/.local/share/applications")
    os.makedirs(share, exist_ok=True)
    path = os.path.join(share, "officework.desktop")
    with open(path, "w", encoding="utf-8") as f:
        f.write(
            "[Desktop Entry]\n"
            "Type=Application\n"
            "Name=officework\n"
            "Comment=表計算と文書\n"
            "Exec={} %f\n"
            "Terminal=false\n"
            "Categories=Office;Spreadsheet;WordProcessor;\n"
            "MimeType={};\n".format(exe, ";".join(MIMES))
        )
    subprocess.run(["update-desktop-database", share], capture_output=True)
    print("アプリの一覧に出しました:")
    print("  ", path)
    return 0


def uninstall_desktop():
    share = os.path.expanduser("~/.local/share/applications")
    n = 0
    # 前の版が置いた2枚も片づける(officework-calc / officework-writer)
    for 名 in ("officework.desktop", "officework-calc.desktop", "officework-writer.desktop"):
        p = os.path.join(share, 名)
        if os.path.exists(p):
            os.remove(p)
            n += 1
    print("{} 件を消しました".format(n))
    return 0


def main():
    """`officework` を打ったとき。**引数があればそのまま窓へ渡す**"""
    args = sys.argv[1:]
    if args and args[0] == "--install-desktop":
        return install_desktop()
    if args and args[0] == "--uninstall-desktop":
        return uninstall_desktop()
    if args and args[0] in ("--help", "-h"):
        return _help()
    return _run(args)


def _help():
    print("officework — 表計算と文書。Python でマクロが書けます。\n")
    print("  officework [ファイル]           窓を開く")
    print("  officework --install-desktop    アプリの一覧に出す(Linux)")
    print()
    print("Python から使う(こちらが主):")
    print("    from officework import calc as xw")
    print("    wb = xw.Book()          # officework が起ち上がる")
    print('    wb.sheets[0]["A1"].value = "こんにちは"')
    print()
    print("ファイルだけ触る(アプリは要りません):")
    print("    from officework import sheet, doc")
    print()
    print("  officework: {}".format(_exe() or "見つかりません"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
