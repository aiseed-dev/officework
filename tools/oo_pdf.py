#!/usr/bin/env python3
"""Convert files with ONLYOFFICE's converter (x2t), as Euro-Office does.

Euro-Office is a fork of ONLYOFFICE, and its file converter is the same x2t
from the `core` repository. This runs the x2t of the official ONLYOFFICE
Desktop Editors Linux package, unpacked in vendor/onlyoffice-desktop (see
tools/onlyoffice_ja.py), to compare how it prints and converts documents
with LibreOffice (`tools/lo_pdf.py`) and with officework.

    python3 tools/oo_pdf.py FILE... [--out DIR] [--to pdf|ods|xlsx|odt|docx] [--x2t PATH]

The default output is `<stem>.oo.pdf` next to each source, or in DIR. With
`--to`, the file is converted to that format instead (`<stem>.<ext>`).

## What we ran into

* Until 2026-10-07 this ran the x2t of the ONLYOFFICE flatpak inside its
  sandbox. The flatpak was removed that day; the unpacked package gives the
  same pixels (below).
* `x2t IN OUT` with two paths converts between file formats, but making a
  PDF goes through the JavaScript editors and needs the font list the app
  builds on its first start (`AllFonts.js`). Without it x2t stops with a V8
  error ("Empty MaybeLocal"). A task file (`TaskQueueDataConvert`) naming
  the font list and a temporary folder makes it work. The font list is read
  from the app's own data folder and never written.
* The format codes are x2t's: 513 PDF, 257 xlsx, 259 ods, 65 docx, 67 odt.
* Each install builds its own font list, with the font paths as that app
  sees them. The flatpak's list named `/run/host/fonts/...`, which exist only
  inside its sandbox. An unpacked package's app builds its list in
  `~/.local/share/onlyoffice` on its first start, with `/usr/share/fonts/...`.
  On 2026-10-07 a docx printed with that list and in the flatpak gave the
  same pixels.
* `--x2t PATH` runs another x2t, for example the one of ja-office-fixes,
  whose word editor has the Japanese patches.
"""
from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
X2T = ROOT / "vendor/onlyoffice-desktop/opt/onlyoffice/desktopeditors/converter/x2t"
# The font list the unpacked app builds on its first start
FONTS = pathlib.Path.home() / ".local/share/onlyoffice/desktopeditors/data/fonts"
CODES = {"pdf": 513, "xlsx": 257, "ods": 259, "docx": 65, "odt": 67}


def convert(src: pathlib.Path, dst: pathlib.Path, to: str, timeout: float, x2t: pathlib.Path = X2T) -> str | None:
    """Convert one file. Returns None on success, or why it failed"""
    fonts = FONTS
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
        cmd = [str(x2t), str(task)]
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
    ap.add_argument("--x2t", default=str(X2T), help="the x2t to run (default: vendor/onlyoffice-desktop's)")
    a = ap.parse_args()
    x2t = pathlib.Path(a.x2t).resolve()
    if not x2t.exists():
        print(f"no x2t at {x2t}: unpack the Linux package as tools/onlyoffice_ja.py says", file=sys.stderr)
        return 2
    if not (FONTS / "AllFonts.js").exists():
        print(f"no font list at {FONTS}: start ONLYOFFICE once to build it", file=sys.stderr)
        return 2
    failed = 0
    for f in a.files:
        src = pathlib.Path(f).resolve()
        out = pathlib.Path(a.out).resolve() if a.out else src.parent
        out.mkdir(parents=True, exist_ok=True)
        dst = out / (f"{src.stem}.oo.pdf" if a.to == "pdf" else f"{src.stem}.{a.to}")
        why = convert(src, dst, a.to, a.timeout, x2t)
        if why:
            failed += 1
            print(f"FAILED {src}: {why}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
