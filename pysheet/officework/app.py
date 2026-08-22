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

**実行ファイルはこの wheel に入っていません**(2026-08-21 発注者。SEKKEI
「officework の wheel からアプリを外す」)。2026-08-15 の同梱の決めを
覆したものです。発注者「officework に画面をいれたのは間違い。ファイルだけに
戻したい」。

pip の `officework` は、**機械に入っている aiseed office を探して起こす**
だけの口です。入っていなければ、入れ方を言って終わります。

画面(aiseed office)は deb / tar.gz / dmg / setup.exe / Flatpak で配ります。
実行ファイルの名前は `officework` のままです — 見せる名前と技術の名前を
分ける決め(SEKKEI「名前の三角形」)。
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
    """機械に入っているアプリを探す。環境変数 → 手元の木 → PATH。

    **wheel の中は見ません**(2026-08-21)。同梱をやめたので、そこには
    ありません。見に行くと「あるはずの所に無い」という遠回りな失敗の仕方に
    なります。
    """
    import shutil

    name = app + (".exe" if sys.platform == "win32" else "")
    here = os.path.dirname(os.path.abspath(__file__))
    # (1) 名指し(別の場所に入れた場合・開発中)
    env = os.environ.get("OFFICEWORK_" + app.upper())
    if env and os.path.exists(env):
        return env
    # (2) 手元の木(officework をソースから触っているとき)
    for up in range(2, 6):
        root = os.path.abspath(os.path.join(here, *([".."] * up)))
        p = os.path.join(root, "target", "release", name)
        if os.path.exists(p):
            return p
    # (3) PATH(deb / dmg / setup.exe / Flatpak で入れた物はここに出ます)
    return shutil.which(name)


# **入れ方**。無いものを「無い」とだけ言って終わらせません
_HOWTO = (
    "画面(aiseed office)がこの機械に入っていません。\n"
    "この pip の荷物は docx / xlsx のファイル操作エンジンで、\n"
    "画面は別に配っています:\n"
    "  https://github.com/aiseed-dev/officework/releases\n"
    "  (Linux は .deb / Flatpak、mac は .dmg、Windows は setup.exe)\n\n"
    "別の場所に入れてあるなら、OFFICEWORK_OFFICEWORK に径路を入れてください。\n\n"
    "ファイルを触るだけなら画面は要りません:\n"
    "    from officework import sheet, doc"
)


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
        sys.exit(_HOWTO)
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
        sys.exit(_HOWTO)
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
