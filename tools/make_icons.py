#!/usr/bin/env python3
"""アプリの絵を SVG 1枚から、配る形へ**全部起こす**。

正本は `packaging/icons/officework-{calc,writer}.svg` の2枚だけ。ここから

    packaging/icons/hicolor/<寸法>/officework-<app>.png   Linux(.deb / .tar.gz)
    packaging/icons/officework-<app>.ico                  Windows
    packaging/icons/officework-<app>.icns                 macOS(.app)

を起こす。**起こした物もコミットする** — 配る仕事(make-linux.sh と
release.yml)に絵を描く道具(inkscape)を要求しないため。CI の runner に
inkscape を入れるより、出来た絵を持っている方が速くて壊れにくい。

    python3 tools/make_icons.py          # 描き直す(inkscape が要る)
    python3 tools/make_icons.py --check  # 揃っているかだけ見る(道具は要らない)

`--check` は CI で回す門番。**絵が1枚も無いのに `.desktop` が
`Icon=officework-calc` と言っている**状態(2026-08-17 のアルファの棚卸しで
見つけた)を二度と作らない。
"""

import argparse
import pathlib
import shutil
import struct
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ICONS = ROOT / "packaging/icons"
APPS = ["calc", "writer"]

# Linux の hicolor に置く寸法。48 と 256 は要(ランチャーと設定画面)
PNG_SIZES = [16, 22, 24, 32, 48, 64, 128, 256, 512]
# Windows の .ico に詰める寸法(大きすぎる物は要らない)
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
# macOS の .icns。型の名前 → 寸法(PNG を包める型だけ使う)
ICNS_TYPES = [(b"ic07", 128), (b"ic08", 256), (b"ic09", 512),
              (b"ic11", 32), (b"ic12", 64), (b"ic13", 256), (b"ic14", 512)]


def png_path(app, size):
    return ICONS / "hicolor" / f"{size}x{size}" / f"officework-{app}.png"


def render(app, size, out):
    out.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["inkscape", "-w", str(size), "-h", str(size), "-o", str(out),
         str(ICONS / f"officework-{app}.svg")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )


def write_icns(pngs, out):
    """PNG を並べて .icns を1つ作る。

    icns は「'icns' + 全体の長さ」の後ろに「型 + 長さ + 中身」が並ぶだけの
    容れ物で、いまの macOS は中身が PNG のままでも読む。**mac が無くても
    作れる**ので、Linux の CI で起こせる(iconutil は mac 専用)。
    """
    body = b""
    for kind, size in ICNS_TYPES:
        data = pngs[size]
        body += kind + struct.pack(">I", len(data) + 8) + data
    out.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)


def check():
    missing = []
    for app in APPS:
        if not (ICONS / f"officework-{app}.svg").exists():
            missing.append(f"officework-{app}.svg(正本)")
        for size in PNG_SIZES:
            if not png_path(app, size).exists():
                missing.append(png_path(app, size).relative_to(ROOT).as_posix())
        for ext in ("ico", "icns"):
            p = ICONS / f"officework-{app}.{ext}"
            if not p.exists():
                missing.append(p.relative_to(ROOT).as_posix())

    # `.desktop` が名指しする絵が実在するか(**ここが本題**)
    sh = (ROOT / "packaging/make-linux.sh").read_text(encoding="utf-8")
    for app in APPS:
        if f"Icon=officework-$app" not in sh and f"Icon=officework-{app}" not in sh:
            missing.append(f"make-linux.sh が officework-{app} を指していない")

    if missing:
        print(f"**アプリの絵が {len(missing)} 件足りません。**")
        print("python3 tools/make_icons.py で起こして、一緒にコミットしてください。\n")
        for m in missing:
            print(f"  {m}")
        return 1
    n = len(APPS) * (len(PNG_SIZES) + 2)
    print(f"アプリの絵は揃っています(正本 {len(APPS)} 枚 → 配る形 {n} 個)")
    return 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="揃っているかだけ見る")
    args = ap.parse_args()
    if args.check:
        return check()

    if not shutil.which("inkscape"):
        print("inkscape がありません(描き直すのに要ります)。")
        print("揃っているかを見るだけなら --check を付けてください。")
        return 1

    from PIL import Image  # 描き直すときだけ要る

    for app in APPS:
        for size in PNG_SIZES:
            render(app, size, png_path(app, size))
        # Windows(.ico)— PIL が複数の寸法を1つに詰める
        Image.open(png_path(app, 512)).save(
            ICONS / f"officework-{app}.ico",
            sizes=[(s, s) for s in ICO_SIZES],
        )
        # macOS(.icns)
        pngs = {s: png_path(app, s).read_bytes() for s in {n for _, n in ICNS_TYPES}}
        write_icns(pngs, ICONS / f"officework-{app}.icns")
        print(f"officework-{app}: PNG {len(PNG_SIZES)} 枚 + .ico + .icns")
    return check()


if __name__ == "__main__":
    sys.exit(main())
