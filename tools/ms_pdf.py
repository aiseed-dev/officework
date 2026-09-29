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
  timeout (two minutes). Wrap the call in `with timeout of 120 seconds`.
* Excel's `open workbook` returns before loading has finished. Looking at
  `active workbook` right away gives missing value. Wait one second at a time
  until the name appears in `workbooks` (a workbook with many formulas takes
  more than 10 seconds).
* Write the output to an ordinary folder such as one under `~/Documents`. Excel
  could not write under `/private/tmp`.
* Word's `open` also returns before the document is there. Waiting one second
  was not enough for a Word template after its folder had just been granted
  access (2026-09-29): `active document` was missing value. Wait until the
  document's name appears, then bring it to the front and save it.
* The first time Word or Excel opens a file in a folder, Office asks on screen
  for access to it ("ファイル アクセスを許可"). Until someone answers, opening
  waits, and the AppleScript only fails with a timeout (-1712) or with
  `active document` not understanding `save as` (-1708).
* So the script looks at Word's and Excel's windows while it waits (Quartz's
  window list, read-only; keystrokes through System Events are not allowed
  here) and stops with a message naming the app and the dialog's title
  (DialogError; exit status 2 from the command line). Answer the dialog on
  screen and run it again. A window is taken as a dialog when its title is a
  known dialog's, or when it is smaller than a document window, its title is
  not the document's name, and it stays up for 8 seconds.
"""
import os
import subprocess
import sys
import time


class DialogError(RuntimeError):
    """Word or Excel is showing a dialog that someone has to answer on screen."""


# Titles of dialogs seen on this Mac. A window with one of these titles is a
# dialog at once; any other small window has to stay up for a while first.
_DIALOG_TITLES = (
    "ファイル アクセスを許可", "Grant File Access",
)


def _app_windows(app):
    """The on-screen windows of `app` ("Microsoft Word" or "Microsoft Excel"),
    read with Quartz (`CGWindowListCopyWindowInfo`).

    This only reads the window list, so it needs no accessibility permission
    (System Events keystrokes are not allowed from osascript on this Mac). The
    titles are empty unless the terminal may record the screen. Returns None
    when pyobjc is missing."""
    try:
        import Quartz
    except ImportError:
        return None
    ws = Quartz.CGWindowListCopyWindowInfo(
        Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements,
        Quartz.kCGNullWindowID)
    out = []
    for w in ws or []:
        if w.get("kCGWindowOwnerName") != app or w.get("kCGWindowLayer", 0) != 0:
            continue
        b = w.get("kCGWindowBounds") or {}
        out.append((int(w.get("kCGWindowNumber", 0)), str(w.get("kCGWindowName") or ""),
                    float(b.get("Width", 0)), float(b.get("Height", 0))))
    return out


def _dialogs(app, doc_names=()):
    """The windows of `app` that look like a dialog: a known dialog title, or a
    window smaller than a document window whose title is not a document's name.

    The window a document opens in is at least 800 by 500 points here; the
    dialogs seen so far (the folder access request, 456 by 299) are smaller."""
    ws = _app_windows(app)
    if not ws:
        return []
    stems = [os.path.splitext(n)[0] for n in doc_names if n]
    out = []
    for num, name, w, h in ws:
        if name and any(s and s in name for s in stems):
            continue
        if name in _DIALOG_TITLES or (w < 800 and h < 500 and w > 60 and h > 60):
            out.append((num, name))
    return out


def _dialog_message(app, found):
    titles = [t for _, t in found if t]
    what = f"「{titles[0]}」" if titles else "(the title could not be read)"
    return (f"{app} is showing a dialog {what}. Answer or close it on screen, "
            f"then run this again. {app} waits for it and does not take further commands")


def check_no_dialog(app, doc_names=()):
    """Raise DialogError when `app` already shows a dialog, before we ask it
    to open anything (otherwise the open waits until the AppleEvent times out)."""
    found = _dialogs(app, doc_names)
    if found:
        raise DialogError(_dialog_message(app, found))


def _osa(script, watch=None, doc_names=(), grace=8.0):
    """Run an AppleScript. With `watch` (the app's name), the app's windows are
    looked at every second while the script runs, and a dialog that stays up
    for `grace` seconds (a known title: 2 seconds) stops the script with
    DialogError. Without this, the script waits until its timeout and fails
    with -1712 without saying why (2026-09-30: a folder access request for a
    new folder)."""
    if watch is None:
        r = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
        if r.returncode != 0:
            raise RuntimeError(r.stderr.strip())
        return r.stdout.strip()
    pr = subprocess.Popen(["osascript", "-e", script], stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, text=True)
    first_seen = {}
    while True:
        try:
            out, err = pr.communicate(timeout=1.0)
            break
        except subprocess.TimeoutExpired:
            pass
        now = time.monotonic()
        found = _dialogs(watch, doc_names)
        seen = {n for n, _ in found}
        for k in list(first_seen):
            if k not in seen:
                del first_seen[k]
        for num, name in found:
            t0 = first_seen.setdefault(num, now)
            limit = 2.0 if name in _DIALOG_TITLES else grace
            if now - t0 >= limit:
                pr.kill()
                pr.communicate()
                raise DialogError(_dialog_message(watch, [(num, name)]))
    if pr.returncode != 0:
        raise RuntimeError(err.strip())
    return out.strip()


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
    name = os.path.basename(src)
    # A dialog already up makes every command below wait: say so and stop
    check_no_dialog("Microsoft Word", [name])
    # Close documents of ours left over from an earlier run (when `save as`
    # failed, the run stopped before closing, and 70 documents piled up in
    # Word on 2026-09-09)
    try:
        word_close_ours()
    except RuntimeError:
        pass
    dialog = None
    try:
        _osa(
            f'''
with timeout of 180 seconds
tell application "Microsoft Word"
    open "{src}"
    -- Word's open returns before the document is there: wait for it
    repeat 90 times
        if (name of every document) contains "{os.path.basename(src)}" then exit repeat
        delay 1
    end repeat
    activate object document "{os.path.basename(src)}"
    delay 1
    save as active document file name "{out}" file format format PDF
end tell
end timeout
''',
            watch="Microsoft Word", doc_names=[name],
        )
    except DialogError as e:
        dialog = e
        raise
    finally:
        # Close whether it worked or not
        try:
            word_close_ours()
        except RuntimeError:
            pass
        # **Stop when a document does not close.** On 2026-09-09 Word began
        # refusing `close` with -1708, nobody noticed, and 100 windows were
        # left open one after another. Count the copies left and have the
        # caller stop if there are any.
        # The count runs on the failure path as well: on 2026-09-19 `save as`
        # raised first, this check was skipped, and 114 documents piled up.
        # With a dialog up, its message is the one that says what to do, so
        # the count does not replace it
        try:
            _nokori_check()
        except RuntimeError:
            if dialog is None:
                raise


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
        # With a dialog up, `close every workbook` waits out its timeout
        check_no_dialog("Microsoft Excel")
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
    check_no_dialog("Microsoft Excel", [name])
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
''',
        watch="Microsoft Excel", doc_names=[name],
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
        except DialogError:
            # Every file after this would wait for the same dialog
            raise
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
    try:
        print(to_pdf(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
    except DialogError as e:
        print(e, file=sys.stderr)
        sys.exit(2)
