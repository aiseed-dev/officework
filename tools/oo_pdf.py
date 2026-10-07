#!/usr/bin/env python3
"""Convert files with ONLYOFFICE's converter (x2t), as Euro-Office does.

Euro-Office is a fork of ONLYOFFICE, and its file converter is the same x2t
from the `core` repository. This runs the x2t of an installed ONLYOFFICE
Desktop Editors flatpak, to compare how it prints and converts documents
with LibreOffice (`tools/lo_pdf.py`) and with officework.

    python3 tools/oo_pdf.py FILE... [--out DIR] [--to pdf|ods|xlsx|odt|docx] [--x2t PATH]

The default output is `<stem>.oo.pdf` next to each source, or in DIR. With
`--to`, the file is converted to that format instead (`<stem>.<ext>`).

## What we ran into

* The flatpak `org.onlyoffice.desktopeditors` keeps x2t at
  `/app/bin/opt/onlyoffice/desktopeditors/converter/x2t`. It runs inside the
  sandbox with `flatpak run --command=…`, and the folders it reads and
  writes are opened to it with `--filesystem=`.
* `x2t IN OUT` with two paths converts between file formats, but making a
  PDF goes through the JavaScript editors and needs the font list the app
  builds on its first start (`AllFonts.js`). Without it x2t stops with a V8
  error ("Empty MaybeLocal"). A task file (`TaskQueueDataConvert`) naming
  the font list and a temporary folder makes it work. The font list is read
  from the app's own data folder and never written.
* The format codes are x2t's: 513 PDF, 257 xlsx, 259 ods, 65 docx, 67 odt.
* Each install builds its own font list, with the font paths as that app
  sees them. The flatpak's list names `/run/host/fonts/...`, which exist only
  inside its sandbox, so the flatpak's x2t runs there.
* `--x2t PATH` runs another x2t directly, for example one in an unpacked
  Linux package (`onlyoffice-desktopeditors-x64.tar.xz`) whose sdkjs was
  replaced. It uses the font list that package's app builds on its first
  start, in `~/.local/share/onlyoffice`, which names `/usr/share/fonts/...`.
  On 2026-10-07 a docx printed this way and in the flatpak gave the same
  pixels.
"""
from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys
import tempfile

APP = "org.onlyoffice.desktopeditors"
X2T = "/app/bin/opt/onlyoffice/desktopeditors/converter/x2t"
FONTS = pathlib.Path.home() / ".var/app" / APP / "data/onlyoffice/desktopeditors/data/fonts"
# The font list of an unpacked Linux package's app
LOCAL_FONTS = pathlib.Path.home() / ".local/share/onlyoffice/desktopeditors/data/fonts"
CODES = {"pdf": 513, "xlsx": 257, "ods": 259, "docx": 65, "odt": 67}


def convert(src: pathlib.Path, dst: pathlib.Path, to: str, timeout: float, x2t: str = X2T) -> str | None:
    """Convert one file. Returns None on success, or why it failed"""
    fonts = FONTS if x2t == X2T else LOCAL_FONTS
    with tempfile.TemporaryDirectory(dir=dst.parent) as tmp:
        task = pathlib.Path(tmp) / "task.xml"
        task.write_text(
            '<?xml version="1.0" encoding="utf-8"?><TaskQueueDataConvert>'
            f"<m_sFileFrom>{src}</m_sFileFrom><m_sFileTo>{dst}</m_sFileTo>"
            f"<m_nFormatTo>{CODES[to]}</m_nFormatTo>"
            f"<m_sAllFontsPath>{fonts / 'AllFonts.js'}</m_sAllFontsPath><m_sFontDir>{fonts}</m_sFontDir>"
            f"<m_sTempDir>{tmp}</m_sTempDir><m_bIsNoBase64>true</m_bIsNoBase64>"
            "</TaskQueueDataConvert>",
            encoding="utf-8",
        )
        if x2t == X2T:
            cmd = ["flatpak", "run", f"--filesystem={src.parent}", f"--filesystem={dst.parent}",
                   f"--command={x2t}", APP, str(task)]
        else:
            cmd = [x2t, str(task)]
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        except subprocess.TimeoutExpired:
            return f"timed out after {timeout} s"
        if not dst.exists():
            return (r.stdout + r.stderr).strip()[-300:] or f"exit {r.returncode}"
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("files", nargs="+")
    ap.add_argument("--out")
    ap.add_argument("--to", default="pdf", choices=sorted(CODES))
    ap.add_argument("--timeout", type=float, default=180)
    ap.add_argument("--x2t", default=X2T, help="the x2t to run (default: the flatpak's own)")
    a = ap.parse_args()
    fonts = FONTS if a.x2t == X2T else LOCAL_FONTS
    if not (fonts / "AllFonts.js").exists():
        print(f"no font list at {fonts}: start ONLYOFFICE once to build it", file=sys.stderr)
        return 2
    failed = 0
    for f in a.files:
        src = pathlib.Path(f).resolve()
        out = pathlib.Path(a.out).resolve() if a.out else src.parent
        out.mkdir(parents=True, exist_ok=True)
        dst = out / (f"{src.stem}.oo.pdf" if a.to == "pdf" else f"{src.stem}.{a.to}")
        x2t = X2T if a.x2t == X2T else str(pathlib.Path(a.x2t).resolve())
        why = convert(src, dst, a.to, a.timeout, x2t)
        if why:
            failed += 1
            print(f"FAILED {src}: {why}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
