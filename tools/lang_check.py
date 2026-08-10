#!/usr/bin/env python3
"""言語まわりを**実機で見る**。

試験は「訳の文字が表に入っているか」までしか見られない。**画面に読める形で
出るか**は別の話で、そこは撮って目で確かめるしかない(2026-08-11、
`ui::language_label` を足したときに要ることが分かった)。

二つの見方がある:

    python3 tools/lang_check.py --pane pt pt-br    # その言語で開いた設定画面
    python3 tools/lang_check.py --list             # 言語欄を順に押していく

`--pane` は `OFFICE_LANG` でその言語に切り替えて撮る。`--list` は控えの
値そのものを変えていくので、**言語欄に出る名前**(`pt-br` ではなく
`Português (Brasil)`)を確かめられる。

前提と作法は tools/ribbon_sweep.py と同じ(X11・私用の HOME・
自分が立てた pid だけ殺す)。**発注者の設定は絶対に触らない。**
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ribbon_sweep as rs  # noqa: E402

OUT = os.path.join(rs.ROOT, "scratch")

# ファイル段は一番左、段の高さは実測でおよそ 30px
FILE_TAB = (40, 42)
# 「詳細設定」は左の列の下から3番目(下に ヘルプ・リクエスト)。
# **実測で決める** — はじめ「下から 96px」と当てずっぽうで書いたら
# 「ヘルプ」のあたりを押していて、何も起きないまま「情報」の画面を
# 撮っていた。撮った絵を見て、高さ 820 の窓で y=691 と数えた
OPTS_FROM_BOTTOM = 129
# 言語欄の札(詳細設定の1行目)
LANG_BOX = (550, 196)


def settings_pane(app):
    """ファイル段 →「詳細設定」まで進む"""
    _, _, _, _, h = app.window()
    app.click(*FILE_TAB)
    time.sleep(0.6)
    # **2回押す。** 1回だと窓に焦点が入るだけで終わることがあり、
    # 「詳細設定」を押したつもりで「情報」の画面を撮っていた
    for _ in range(2):
        app.click(90, h - OPTS_FROM_BOTTOM)
        time.sleep(0.5)
    time.sleep(0.5)


def pane(lang, where="opts"):
    """その言語で立てて、ファイル段の画面を撮る(既定は詳細設定)"""
    os.environ["OFFICE_LANG"] = lang
    app = rs.App(shots=OUT)
    try:
        if where == "info":
            # ここも2回。1回目は窓に焦点が入るだけのことがある
            for _ in range(2):
                app.click(*FILE_TAB)
                time.sleep(0.6)
        else:
            settings_pane(app)
        return app.shot(f"lang-{where}-{lang}")
    finally:
        app.close()


def funcs(lang):
    """関数の一覧の小窓をその言語で撮る。**説明が出る唯一の場所**なので、
    ここを見ないと訳が繋がったか分からない"""
    os.environ["OFFICE_LANG"] = lang
    app = rs.App(shots=OUT)
    try:
        # **数式バーの fx を押す。** リボンの段から辿ると、段の並びが
        # 言語で変わるので座標が当てにならない(2026-08-11、データ段を
        # 押していて空の表を撮っていた)。fx はどの言語でも同じ場所
        app.click(125, 138)
        time.sleep(1.4)
        return app.shot(f"lang-fn-{lang}")
    finally:
        app.close()


def listing(times):
    """言語欄を順に押して、名前が変わっていくところを撮る。

    押すと控えが変わるが、書き込み先は**私用の HOME** なので
    発注者の settings.toml には届かない(rs.App が分けている)。
    """
    os.environ.pop("OFFICE_LANG", None)
    app = rs.App(shots=OUT)
    shots = []
    try:
        settings_pane(app)
        shots.append(app.shot("lang-list-00"))
        for i in range(1, times + 1):
            app.click(*LANG_BOX)
            time.sleep(0.4)
            shots.append(app.shot(f"lang-list-{i:02}"))
        return shots
    finally:
        app.close()


def main():
    os.makedirs(OUT, exist_ok=True)
    args = sys.argv[1:]
    if "--list" in args:
        args.remove("--list")
        n = int(args[0]) if args else len(rs_languages())
        for p in listing(n):
            print(f"  {p}")
        return
    if args and args[0] == "--funcs":
        for lang in args[1:] or ["ja"]:
            print(f"{lang}: {funcs(lang)}")
        return
    where = "opts"
    if args and args[0] == "--info":
        where, args = "info", args[1:]
    elif args and args[0] == "--pane":
        args = args[1:]
    for lang in args or ["ja", "pt", "pt-br", "en"]:
        print(f"{lang}: {pane(lang, where)}")


def rs_languages():
    """登録済みの言語の数(ja を含む)。表から数えて当て推量を避ける"""
    src = open(os.path.join(rs.ROOT, "lang/src/i18n_tables.rs"),
               encoding="utf-8").read()
    body = src.split("LANGS: &[&str] = &[")[1].split("]")[0]
    return [s.strip(' "') for s in body.split(",") if s.strip()] + ["ja"]


if __name__ == "__main__":
    main()
