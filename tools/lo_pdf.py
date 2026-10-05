#!/usr/bin/env python3
"""Open files in LibreOffice and save them as PDF, or convert them to ODF.

The LibreOffice counterpart of `tools/ms_pdf.py`. It makes the reference PDFs
that the PDFs officework writes itself are compared against (with
`tools/ms_compare.py`). It also turns the xlsx / docx corpus into ODF test
documents.

    python3 tools/lo_pdf.py FILE... [--out DIR] [--to pdf|ods|odt|xlsx|docx]
                            [--profile DIR] [--timeout SECONDS]

Accepted inputs: ods, odt, xlsx, docx, csv, fods, fodt.

* PDF (the default): a `.lo.pdf` with the same name is written next to each
  source file (`ms_pdf.py` writes `.ms.pdf`). With `--out DIR`, `<stem>.lo.pdf`
  is written into DIR.
* `--to ods` converts spreadsheets (ods, xlsx, csv, fods) to `<stem>.ods`.
  `--to odt` converts documents (odt, docx, fodt) to `<stem>.odt`. `--to xlsx`
  and `--to docx` convert the same inputs to OOXML, to check how LibreOffice
  read a file by what it writes back. The output
  goes next to the source, or into DIR with `--out`. An existing output file is
  overwritten.

The LibreOffice version is printed once at the start, because the reference
PDFs depend on it. The exit status is 1 when any file produced no output, and
2 for a wrong command line.

## What we ran into

* Every call to soffice gets its own profile directory
  (`-env:UserInstallation=file://<dir>`): a fresh folder under the system temp
  directory, removed at the end, or the folder given with `--profile`. Without
  it, soffice uses the user's default profile, and if LibreOffice is already
  running in that profile the call hands the file to that window and exits
  without converting anything. The own profile keeps this tool away from
  running LibreOffice windows and from the user's settings.
* soffice names the output after the input (`<stem>.pdf`) and writes it into
  `--outdir`. Two inputs with the same stem (`a/x.xlsx` and `b/x.xlsx`, or
  `x.xlsx` and `x.csv`) would overwrite each other. The files are therefore
  split into groups with no repeated stem, each group is converted by one soffice
  call into its own temporary folder, and the result is moved to its final
  name afterwards.
* Converting `x.ods` with `--to ods` would write `x.ods` over its own source if
  the output folder were the source folder. The temporary folder avoids this,
  and the source is only replaced when the output path is the same file on
  purpose.
* Many inputs can go to one soffice call, which saves the start-up time of
  about a second per file. If a call fails or times out, the files it did not
  finish are tried again one by one, so one broken file does not cost the rest.
  The exit status of soffice is not trusted: a conversion that failed can still
  exit with 0. A file counts as converted only when the output file exists.
* `soffice` is a shell script that starts `soffice.bin`. Killing only the
  script on a timeout leaves `soffice.bin` running. Each call starts in its own
  process group and the whole group is killed. Only processes this tool
  started are killed; a pgrep loop is not used (see CLAUDE.md).
* The PDF filter is picked from the input type by `--convert-to pdf` (a csv
  opens in Calc, a docx in Writer). The page size, the print ranges and the
  page style are the ones stored in the document, as they would be when printing.
* soffice prints to stdout and stderr in a form that is not stable between
  versions. Only the output file is checked, and the last lines of its
  messages are shown when a file failed.
* A file name that starts with `-` would be read as a soffice option, so such
  paths are made absolute first.

* `tools/ms_compare.py` reads LibreOffice PDFs without changes (it uses only the
  characters and their baseline positions, through pdfplumber). Comparing a
  `.lo.pdf` with itself gives 0 differences.
* Calc's default page style prints the sheet name as a header and "Page N" as
  a footer. A csv converted with no page style of its own therefore has the
  sheet name (the file stem) as its first line. Word and Excel print no such
  lines, so expect them as differences.
* A file that is not really an xlsx (text named `.xlsx`) is not an error for
  LibreOffice: it falls back to the text import and writes a PDF of the text.
  "A PDF exists" does not mean "the file was valid".
* A formula with a function LibreOffice does not have (officework's own
  functions) shows `#NAME?` in the PDF. That is LibreOffice's answer, not a
  failure of this tool.
* Two sources whose output would be the same file (the same stem in one output
  folder: `a/x.xlsx` and `b/x.xlsx` with `--out`, or `x.csv` and `x.xlsx` next to
  each other) are refused up front with a message, since the second would
  overwrite the first. Run them separately or give different folders.
"""
import argparse
import os
import shutil
import signal
import subprocess
import sys
import tempfile
from pathlib import Path

SHEET_EXTS = (".ods", ".xlsx", ".csv", ".fods")
TEXT_EXTS = (".odt", ".docx", ".fodt")
ALL_EXTS = SHEET_EXTS + TEXT_EXTS

# How the output is named, and which inputs each target accepts.
TARGETS = {
    "pdf": {"suffix": ".lo.pdf", "ext": "pdf", "accepts": ALL_EXTS},
    "ods": {"suffix": ".ods", "ext": "ods", "accepts": SHEET_EXTS},
    "odt": {"suffix": ".odt", "ext": "odt", "accepts": TEXT_EXTS},
    "xlsx": {"suffix": ".xlsx", "ext": "xlsx", "accepts": SHEET_EXTS},
    "docx": {"suffix": ".docx", "ext": "docx", "accepts": TEXT_EXTS},
}


def soffice_bin():
    exe = shutil.which("soffice") or shutil.which("libreoffice")
    if not exe:
        raise SystemExit("soffice was not found on PATH")
    return exe


def version(exe):
    r = subprocess.run([exe, "--version"], capture_output=True, text=True, timeout=60)
    return (r.stdout or r.stderr).strip()


def plan(sources, target, out_dir):
    """Return a list of (source, final output path). Two sources that would get
    the same final path are an error, since the second would overwrite the first."""
    t = TARGETS[target]
    jobs, seen = [], {}
    for s in sources:
        src = Path(s).resolve()
        dst_dir = Path(out_dir).resolve() if out_dir else src.parent
        dst = dst_dir / (src.stem + t["suffix"])
        if dst in seen:
            raise SystemExit(f"{seen[dst]} and {src} would both be written to {dst}; "
                             "run them separately or use different folders")
        seen[dst] = src
        jobs.append((src, dst))
    return jobs


def groups_by_stem(jobs):
    """Split jobs into groups in which every source stem is unique, so that
    soffice's `<stem>.<ext>` outputs of one call cannot collide."""
    groups = []
    for job in jobs:
        for g in groups:
            if all(job[0].stem != o[0].stem for o in g):
                g.append(job)
                break
        else:
            groups.append([job])
    return groups


def run_soffice(exe, profile, target, srcs, outdir, timeout):
    """One soffice call for `srcs`. Returns (finished, message). On timeout the
    whole process group is killed (soffice is a script that starts soffice.bin)."""
    cmd = [exe, f"-env:UserInstallation={Path(profile).resolve().as_uri()}",
           "--headless", "--norestore", "--nologo", "--nolockcheck",
           "--convert-to", TARGETS[target]["ext"], "--outdir", str(outdir),
           *[str(s) for s in srcs]]
    pr = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          text=True, start_new_session=True)
    try:
        out, _ = pr.communicate(timeout=timeout)
        return True, out
    except subprocess.TimeoutExpired:
        try:
            os.killpg(pr.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        out, _ = pr.communicate()
        return False, (out or "") + f"\n(timed out after {timeout:g} s)"


def convert(exe, profile, target, jobs, timeout_per_file):
    """Convert all jobs. Returns a list of (source, error or None)."""
    results = {}
    for group in groups_by_stem(jobs):
        tmp = Path(tempfile.mkdtemp(prefix="lo_pdf_out_"))
        try:
            ext = TARGETS[target]["ext"]
            ok, msg = run_soffice(exe, profile, target, [j[0] for j in group], tmp,
                                  timeout_per_file * len(group))
            last = {}
            for src, dst in group:
                made = tmp / f"{src.stem}.{ext}"
                if not made.exists() and len(group) > 1:
                    # A broken file can stop the whole call: try this one alone
                    ok1, msg1 = run_soffice(exe, profile, target, [src], tmp, timeout_per_file)
                    last[src] = msg1
                if made.exists():
                    dst.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(made), str(dst))
                    results[src] = None
                else:
                    tail = [ln for ln in (last.get(src) or msg).splitlines() if ln.strip()][-3:]
                    results[src] = "no output" + (": " + " / ".join(tail) if tail else "")
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
    return [(j[0], results[j[0]]) for j in jobs]


def main(argv=None):
    ap = argparse.ArgumentParser(description="Convert files with LibreOffice (headless, own profile).")
    ap.add_argument("files", nargs="+", help="ods, odt, xlsx, docx, csv, fods, fodt")
    ap.add_argument("--out", metavar="DIR", help="write outputs into DIR instead of next to the sources")
    ap.add_argument("--to", choices=sorted(TARGETS), default="pdf",
                    help="pdf (default, writes <stem>.lo.pdf), ods or odt (convert to ODF), "
                         "xlsx or docx (convert to OOXML)")
    ap.add_argument("--profile", metavar="DIR",
                    help="LibreOffice profile folder to use (default: a new temporary one, removed at the end)")
    ap.add_argument("--timeout", type=float, default=120.0, metavar="SECONDS",
                    help="allowed time per file (default 120); a call with N files gets N times this")
    a = ap.parse_args(argv)

    accepts = TARGETS[a.to]["accepts"]
    bad = [f for f in a.files if Path(f).suffix.lower() not in accepts]
    if bad:
        print(f"--to {a.to} takes {', '.join(accepts)}; not accepted: {', '.join(bad)}", file=sys.stderr)
        return 2
    missing = [f for f in a.files if not Path(f).is_file()]
    if missing:
        print(f"not found: {', '.join(missing)}", file=sys.stderr)
        return 2

    exe = soffice_bin()
    print(version(exe), flush=True)
    jobs = plan(a.files, a.to, a.out)

    own_profile = a.profile is None
    profile = Path(tempfile.mkdtemp(prefix="lo_pdf_profile_")) if own_profile else Path(a.profile)
    profile.mkdir(parents=True, exist_ok=True)
    try:
        results = convert(exe, profile, a.to, jobs, a.timeout)
    finally:
        if own_profile:
            shutil.rmtree(profile, ignore_errors=True)

    failed = 0
    for (src, dst), (_, err) in zip(jobs, results):
        if err:
            failed += 1
            print(f"FAILED {src}: {err}", file=sys.stderr)
        else:
            print(dst)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
