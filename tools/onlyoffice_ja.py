#!/usr/bin/env python3
"""Build and start ONLYOFFICE Desktop Editors with the Japanese line-breaking patches.

The patches in docs/sekkei/euro-office-nihongo/patches change only sdkjs, the
editor engine. This script applies them to ONLYOFFICE's sdkjs at the tag of
the desktop release, builds the word editor, and puts the result into an
unpacked Linux package (onlyoffice-desktopeditors-x64.tar.xz).

    python3 tools/onlyoffice_ja.py build     # apply the patches and build sdkjs
    python3 tools/onlyoffice_ja.py install   # put the build into the package
    python3 tools/onlyoffice_ja.py restore   # put the original sdkjs back
    python3 tools/onlyoffice_ja.py run [FILE...]

Folders (git-ignored):

* vendor/onlyoffice-sdkjs: `git clone --depth 1 --branch v9.4.0.129
  https://github.com/ONLYOFFICE/sdkjs`. The tag is the build of the desktop
  release (the header of its sdk-all-min.js says "build:129").
* vendor/onlyoffice-desktop: the tar.xz from the DesktopEditors release,
  unpacked. The original word sdkjs is kept in orig-sdkjs-word/ there.

## What we ran into

* The sdkjs of 9.4 builds with build/build.py, which only joins the files;
  no Closure Compiler and no npm are needed.
* sdk-all.bin next to sdk-all.js is a cache of the original script, and the
  app writes sdk-all.cache there when it runs. Both are removed whenever a
  script goes in, so a stale cache is never used.
* The app starts only from its own folder with LD_LIBRARY_PATH=./, as the
  flatpak's launcher does (`cd`, then `./DesktopEditors`). Without it, Qt
  cannot load its xcb plugin.
"""
from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
SDKJS = ROOT / "vendor/onlyoffice-sdkjs"
PKG = ROOT / "vendor/onlyoffice-desktop"
APP = PKG / "opt/onlyoffice/desktopeditors"
WORD = APP / "editors/sdkjs/word"
ORIG = PKG / "orig-sdkjs-word"
PATCHES = ROOT / "docs/sekkei/euro-office-nihongo/patches"
BRANCH = "ja-line-break"


def git(*args: str) -> str:
    return subprocess.run(["git", "-C", str(SDKJS), *args], check=True, capture_output=True, text=True).stdout


def build() -> None:
    branches = git("branch", "--list", BRANCH)
    if not branches.strip():
        git("switch", "-q", "-c", BRANCH)
        patches = sorted(str(p) for p in PATCHES.glob("*.patch"))
        subprocess.run(["git", "-C", str(SDKJS), "am", "-q", "--3way", *patches], check=True)
    else:
        git("switch", "-q", BRANCH)
    subprocess.run([sys.executable, "build.py", "--product", "word", "--desktop"], cwd=SDKJS / "build", check=True)
    print("built", SDKJS / "deploy/sdkjs/word")


def install() -> None:
    if not ORIG.exists():
        ORIG.mkdir(parents=True)
        for f in WORD.iterdir():
            shutil.copy2(f, ORIG / f.name)
    for name in ("sdk-all-min.js", "sdk-all.js"):
        shutil.copy2(SDKJS / "deploy/sdkjs/word" / name, WORD / name)
    for cache in ("sdk-all.bin", "sdk-all.cache"):
        (WORD / cache).unlink(missing_ok=True)
    print("installed the patched sdkjs into", WORD)


def restore() -> None:
    (WORD / "sdk-all.cache").unlink(missing_ok=True)
    for f in ORIG.iterdir():
        shutil.copy2(f, WORD / f.name)
    print("restored the original sdkjs into", WORD)


def run(files: list[str]) -> None:
    env = dict(os.environ, LD_LIBRARY_PATH="./", QT_QPA_PLATFORM="xcb")
    paths = [str(pathlib.Path(f).resolve()) for f in files]
    subprocess.Popen(["./DesktopEditors", *paths], cwd=APP, env=env, start_new_session=True,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def main() -> int:
    cmd, rest = (sys.argv[1], sys.argv[2:]) if len(sys.argv) > 1 else ("", [])
    actions = {"build": build, "install": install, "restore": restore}
    if cmd in actions:
        actions[cmd]()
    elif cmd == "run":
        run(rest)
    else:
        print(__doc__.split("\n\n")[1])
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
