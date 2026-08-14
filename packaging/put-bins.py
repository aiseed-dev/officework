#!/usr/bin/env python3
"""組んだ calc / writer を wheel に載せる場所へ置く。

    cargo build --release -p calc -p writer
    python3 packaging/put-bins.py
    cd pysheet && maturin build --release

**strip する。** 実測(2026-08-15)で calc は 60.5MB → strip 44.1MB →
wheel の中(zip)で 16.3MB。2本で約 32MB なので PyPI の 100MB に収まる。

なぜ PyPI で配るのか: **署名が要らない**。mac の Gatekeeper も Windows の
SmartScreen も「ブラウザで落とした物」に付く印(quarantine / Mark of the
Web)が引き金で、pip が入れた物には付かない。Python 同梱も要らない
(pip で入れる人は Python を持っている)。
"""
import os
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEST = ROOT / "pysheet" / "officework" / "bin"
APPS = ("calc", "writer")


def main():
    exe_suffix = ".exe" if sys.platform == "win32" else ""
    DEST.mkdir(parents=True, exist_ok=True)
    placed = []
    for app in APPS:
        src = ROOT / "target" / "release" / (app + exe_suffix)
        if not src.exists():
            sys.exit(
                f"{src} がありません。先に組んでください:\n"
                f"  cargo build --release -p calc -p writer"
            )
        dst = DEST / (app + exe_suffix)
        shutil.copy2(src, dst)
        # strip(Windows の .exe は既に符号が入っていないので飛ばす)
        if sys.platform != "win32" and shutil.which("strip"):
            subprocess.run(["strip", str(dst)], check=False)
        os.chmod(dst, 0o755)
        placed.append((dst, dst.stat().st_size))
    print("wheel に載せる実行ファイル:")
    for p, n in placed:
        print(f"  {p.name}  {n / 1048576:.1f}MB")
    print(f"置き場: {DEST}")


if __name__ == "__main__":
    main()
