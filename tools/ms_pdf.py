#!/usr/bin/env python3
"""Open a file in Word or Excel and save it as PDF (macOS).

Takes a docx or xlsx that officework wrote, has the real Word or Excel open it,
and saves it as PDF. The result is the material for comparing page breaks, line
wrapping and how values look against the PDF officework writes itself
(`tools/ms_compare.py` does the comparison).

    python3 tools/ms_pdf.py document.docx [output.pdf]
    python3 tools/ms_pdf.py ledger.xlsx [output.pdf]

If the output is left out, a `.ms.pdf` with the same name is written next to
the source file.

## What we ran into

* Word and Excel are driven with AppleScript (`osascript`). The first time,
  macOS asks whether to allow automation, so allow it on screen.
* Word's `save as` is written `file format format PDF`, while Excel's `save as`
  is written `file format PDF file format`. The two differ.
* Excel's `open` only accepts a path in the form "Macintosh HD:Users:…". Given a
  POSIX path it opens nothing and returns silently. Word accepts a POSIX path.
* To stop the alerts shown while opening (compatibility mode, updating links),
  Excel has `display alerts` turned off. In that state a broken file is silently
  not opened (`active workbook` stays missing value). On 2026-09-08 this is how
  we found an xlsx that had only the theme relationship and was missing its
  parts.
* Word's `save as` writes the front document, not the one you named. Running
  five documents by name with all five open produced the same PDF five times.
  Open one at a time, write `active document`, then close it before the next.
* Opening can take more than two minutes, which hits the default AppleEvent
  timeout (two minutes). Wrap the call in `with timeout of 600 seconds`.
* Excel's `open workbook` returns before loading has finished. Looking at
  `active workbook` right away gives missing value. Wait one second at a time
  until the name appears in `workbooks` (a workbook with many formulas takes
  more than 10 seconds).
* Write the output to an ordinary folder such as one under `~/Documents`. Excel
  could not write under `/private/tmp`.
"""
import os
import subprocess
import sys


def _osa(script):
    r = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.strip())
    return r.stdout.strip()


def _hfs(path):
    """Turn a POSIX path into the form "Macintosh HD:Users:…". Excel's open only
    accepts this form (given a POSIX path it opens nothing and returns silently).
    Writing `POSIX file` inside the tell block sends it to Excel and fails with
    -50, so the conversion is done separately here."""
    return _osa(f'POSIX file "{os.path.abspath(path)}" as string')


def word_close_ours():
    """Close every document we had opened (the ones whose path contains
    officework-cmp). The owner's own documents are left alone. The owner decided on
    2026-09-09 that files opened on screen must be closed as the work goes along."""
    # `repeat with d in (every document)` は Word が -1708 で断る(Excel の
    # `repeat with w in workbooks` が -50 なのと同じ癖)。名前を先に取り、名指しで閉じる
    _osa('''
with timeout of 120 seconds
tell application "Microsoft Word"
    repeat with i from (count documents) to 1 by -1
        try
            set d to document i
            if (full name of d) contains "officework-cmp" then close d saving no
        end try
    end repeat
end tell
end timeout
''')


def word_pdf(src, out):
    src, out = os.path.abspath(src), os.path.abspath(out)
    # 前の回で閉じ損ねた文書が残っていれば先に閉じる(`save as` が失敗すると
    # 閉じる前に止まり、Word に文書が溜まっていった。2026-09-09 に 70 枚溜まった)
    try:
        word_close_ours()
    except RuntimeError:
        pass
    try:
        _osa(
            f'''
with timeout of 600 seconds
tell application "Microsoft Word"
    open "{src}"
    delay 1
    set d to active document
    save as d file name "{out}" file format format PDF
end tell
end timeout
'''
        )
    finally:
        # 成否にかかわらず閉じる
        try:
            word_close_ours()
        except RuntimeError:
            pass
        # **閉じられなかったら止まる。** 2026-09-09、Word が `close` を -1708 で断る
        # 状態になったのに気づかず、100 枚の窓を開いたまま次々に進んでしまった。
        # 残っている写しを数え、残っていれば呼ぶ側に止めてもらう。
        # The count runs on the failure path as well: on 2026-09-19 `save as`
        # raised first, this check was skipped, and 114 documents piled up
        _nokori_check()


def _nokori_check():
    """Raise when Word still holds documents of ours; the caller must stop."""
    nokori = _osa('''
with timeout of 60 seconds
tell application "Microsoft Word"
    set n to 0
    repeat with i from 1 to (count documents)
        try
            if (full name of document i) contains "officework-cmp" then set n to n + 1
        end try
    end repeat
    n
end tell
end timeout
''')
    if nokori.strip() not in ("", "0"):
        raise RuntimeError(f"Word が文書を閉じません({nokori.strip()} 枚残っています)。これ以上は開かずに止めます")


def _excel_ready():
    """Put Excel into a state where AppleScript's `open workbook` and saving a PDF
    both work.

    What we ran into (2026-09-08, the combination found over a whole day):
    * A workbook opened through the Finder (`open -a` with a file) makes `save as`
      write nothing at all, and `close` has no effect either. While that workbook
      is around, AppleScript's `open workbook` is refused with -50. Do not open the
      file you want with `open -a`.
    * Quitting quietly with `quit saving no` makes the next launch show the start
      screen (the list of templates), and `make new workbook` then waits 2 minutes.
    * The order that worked: close every open workbook, `killall`, `open -a` with
      no file, `make new workbook` (an empty workbook so there is a window), then
      AppleScript's `open workbook` (a Macintosh HD: path), then `save as … PDF`.
      Quitting while an unsaved workbook is still open makes the next launch show
      the recovery screen and go wrong, so close it first.
    """
    import time

    r = subprocess.run(["pgrep", "-f", "MacOS/Microsoft Excel"], capture_output=True, text=True)
    if r.stdout.strip():
        subprocess.run(["osascript", "-e", 'tell application "Microsoft Excel" to close every workbook saving no'],
                       capture_output=True, text=True, timeout=120)
        subprocess.run(["killall", "Microsoft Excel"], capture_output=True, text=True)
        for _ in range(30):
            r = subprocess.run(["pgrep", "-f", "MacOS/Microsoft Excel"], capture_output=True, text=True)
            if not r.stdout.strip():
                break
            time.sleep(1)
    subprocess.run(["open", "-a", "Microsoft Excel"], check=False)
    time.sleep(10)


def excel_pdf(src, out):
    """Have Excel open the file and save the visible sheet as PDF.

    The steps that worked (2026-09-08; of three approaches tried, only this one
    actually wrote a file):
    1. First make one empty workbook so there is a window (without a window,
       `open workbook` fails with -50).
    2. Have it open the file by passing a "Macintosh HD:…" path to
       `open workbook workbook file name`. A workbook opened with `open -a`
       (through the Finder) makes `save as` to another path write nothing at all
       (probably because of the sandbox).
    3. Wait until the name appears in `workbooks` (open returns before loading).
    4. `save as active sheet … file format PDF file format`. The destination is a
       POSIX path.
    """
    _excel_ready()
    _excel_one(src, out)


def _excel_one(src, out, wait=180):
    """開いている Excel に1枚だけ開かせて PDF にします。

    `_excel_ready()` が済んでいることが前提です。空のブック(窓を持たせる
    ため)は毎回作って毎回閉じます。まとめて回すときは
    [`excel_pdf_many`] を使ってください — Excel の起動と終了は1回で済みます。
    """
    name = os.path.basename(src)
    hfs, out = _hfs(src), os.path.abspath(out)
    _osa(
        f'''
with timeout of 600 seconds
tell application "Microsoft Excel"
    set display alerts to false
    -- 空のブックで窓を持たせる。最初の画面が出ていると待たされるので 10 秒で切って繰り返す
    set nb to missing value
    repeat 20 times
        try
            with timeout of 10 seconds
                set nb to make new workbook
            end timeout
            exit repeat
        on error
            delay 3
        end try
    end repeat
    if nb is missing value then error "Excel が空のブックを作れませんでした(最初の画面が消えない)"
    open workbook workbook file name "{hfs}"
    set wb to missing value
    repeat {wait} times
        try
            if (name of every workbook) contains "{name}" then set wb to workbook "{name}"
        end try
        if wb is not missing value then exit repeat
        delay 1
    end repeat
    if wb is missing value then error "Excel が {wait} 秒たっても開きませんでした(壊れたファイルと見た可能性): {hfs}"
    activate object wb
    save as active sheet filename "{out}" file format PDF file format
    close wb saving no
    close nb saving no
    set display alerts to true
end tell
end timeout
'''
    )


def excel_pdf_many(pairs, wait=120, on_done=None):
    """Convert several files to PDF in one run. `pairs` is a list of
    (path of the xlsx, PDF to write).

    Excel is quit and restarted only once, at the start. That saves about 30
    seconds per file, which matters when running through a whole collection of
    xlsx files. A failure on one file does not stop the rest. The return value is
    a list of (path, error message or None).

    Once Excel goes wrong, everything after it fails, so it is restarted after
    three failures in a row. If `on_done` is given, it is called once per file
    with (path, error).
    """
    _excel_ready()
    out = []
    renzoku = 0
    for src, dst in pairs:
        try:
            _excel_one(src, dst, wait=wait)
            err = None if os.path.exists(dst) else "PDF ができていない"
        except Exception as e:  # noqa: BLE001 — 1枚の失敗で残りを止めない
            err = str(e).splitlines()[-1][:200] if str(e).strip() else type(e).__name__
        renzoku = renzoku + 1 if err else 0
        out.append((src, err))
        if on_done:
            on_done(src, err)
        if renzoku >= 3:
            _excel_ready()
            renzoku = 0
    _excel_quit()
    return out


def _excel_quit():
    """自分が起動した Excel を閉じます(発注者の Word には触りません)。"""
    subprocess.run(["osascript", "-e",
                    'tell application "Microsoft Excel" to close every workbook saving no'],
                   capture_output=True, text=True, timeout=120)
    subprocess.run(["killall", "Microsoft Excel"], capture_output=True, text=True)


def to_pdf(src, out=None):
    if out is None:
        base, _ = os.path.splitext(src)
        out = base + ".ms.pdf"
    ext = os.path.splitext(src)[1].lower()
    if ext in (".docx", ".doc", ".adoc"):
        word_pdf(src, out)
    elif ext in (".xlsx", ".xlsm", ".xls"):
        excel_pdf(src, out)
    else:
        raise SystemExit(f"docx か xlsx を渡してください: {src}")
    if not os.path.exists(out):
        raise SystemExit(f"PDF ができていません: {out}")
    return out


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    print(to_pdf(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
