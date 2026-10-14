#!/usr/bin/env python3
"""Have Word / Excel in the Windows VM save docx / xlsx files as PDF (Linux).

The Linux counterpart of `tools/ms_pdf.py` (which drives Word and Excel on a
Mac). The PDFs are the material `tools/ms_compare.py` compares with the PDF
officework writes.

    python3 tools/winvm_pdf.py document.docx ledger.xlsx [--out DIR] [--timeout 300] [--keep]

Each `<name>.docx` / `<name>.xlsx` gives `<name>.ms.pdf`, next to the source
file or in `--out`. The exit status is 0 when every file gave a PDF, 1 when
some did not, and 2 when Word or Excel could not be closed (see below).

## The VM

A libvirt domain (default `win11` on `qemu:///session`; `OW_WINVM_DOMAIN` and
`OW_WINVM_CONNECT` change them) with Word and Excel installed, a user signed
in, and the QEMU guest agent running: install `virtio-win-guest-tools.exe`
from the virtio-win ISO in Windows, and give the domain the channel
`<channel type='unix'><target type='virtio' name='org.qemu.guest_agent.0'/></channel>`.

## Starting and shutting down the VM

The VM is meant to run only while comparing (it holds its memory until it is
shut down). If it is not running, the tool starts it, waits until someone
signs in to Windows (the tool does not sign in), and shuts Windows down when
done. A VM that was already running is left running, since it may be in use;
`--keep` leaves a VM the tool started running too.

## How it works

* Files go in and out through the guest agent (`guest-file-open` / `-write` /
  `-read`), under `C:\\owcmp` in the guest.
* The guest agent runs commands as SYSTEM, which has no Office licence or user
  profile. So `tools/winvm/ms_pdf.ps1` runs as the signed-in user through a
  scheduled task (`LogonType Interactive`, no password needed). The user's name
  is read from Windows each time.
* If Word or Excel is already running, nothing is started: it may be a window
  the user is working in.
* When the run does not finish in time, `tools/winvm/close.ps1` closes the Word
  and Excel this run started, the way the window's close button does, or over
  COM for a hidden one. They are never killed: a killed Word leaves recovered
  files, and the next hidden Word waits for ever on the question about them.
  If they are still running after that, the script stops and says so.
"""
import argparse
import base64
import json
import os
import subprocess
import sys
import time
from pathlib import Path

CONNECT = os.environ.get("OW_WINVM_CONNECT", "qemu:///session")
DOMAIN = os.environ.get("OW_WINVM_DOMAIN", "win11")
ROOT = r"C:\owcmp"
HERE = Path(__file__).resolve().parent / "winvm"
TASK = "officework-ms-pdf"
TASK_CLOSE = "officework-ms-pdf-close"
CHUNK = 48 * 1024


class VmError(RuntimeError):
    pass


def agent(cmd, **args):
    req = {"execute": cmd}
    if args:
        req["arguments"] = args
    r = subprocess.run(
        ["virsh", "-c", CONNECT, "qemu-agent-command", DOMAIN, json.dumps(req), "--timeout", "60"],
        capture_output=True, text=True)
    if r.returncode != 0:
        raise VmError(r.stderr.strip() or r.stdout.strip())
    return json.loads(r.stdout)["return"]


def ps(script, timeout=120):
    """Run PowerShell in the guest as SYSTEM. Returns (exit code, stdout, stderr)."""
    pid = agent("guest-exec", path="powershell.exe", **{
        "arg": ["-NoProfile", "-NonInteractive", "-Command",
                "[Console]::OutputEncoding=[Text.Encoding]::UTF8;" + script],
        "capture-output": True})["pid"]
    t0 = time.time()
    while True:
        st = agent("guest-exec-status", pid=pid)
        if st.get("exited"):
            break
        if time.time() - t0 > timeout:
            raise VmError(f"a guest command took more than {timeout}s")
        time.sleep(0.5)
    dec = lambda k: base64.b64decode(st.get(k, "")).decode("utf-8", "replace")
    return st.get("exitcode"), dec("out-data"), dec("err-data")


def ps_ok(script, timeout=120):
    code, out, err = ps(script, timeout)
    if code:
        raise VmError(f"guest command failed ({code}): {err.strip() or out.strip()}")
    return out


def put(data, remote):
    h = agent("guest-file-open", path=remote, mode="wb")
    try:
        for i in range(0, len(data), CHUNK):
            agent("guest-file-write", handle=h,
                  **{"buf-b64": base64.b64encode(data[i:i + CHUNK]).decode()})
    finally:
        agent("guest-file-close", handle=h)


def get(remote):
    h = agent("guest-file-open", path=remote, mode="rb")
    out = []
    try:
        while True:
            r = agent("guest-file-read", handle=h, count=CHUNK)
            out.append(base64.b64decode(r["buf-b64"]))
            if r.get("eof") or not r["count"]:
                break
    finally:
        agent("guest-file-close", handle=h)
    return b"".join(out)


def put_script(name):
    """Copy a script from tools/winvm into the guest, as UTF-8 with a BOM and
    CRLF so Windows PowerShell 5.1 reads the Japanese in it correctly."""
    text = (HERE / name).read_text(encoding="utf-8").replace("\n", "\r\n")
    put(b"\xef\xbb\xbf" + text.encode("utf-8"), ROOT + "\\" + name)


def register(task, script, args, user):
    ps_ok(f"""
$a = New-ScheduledTaskAction -Execute powershell.exe -Argument '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File {ROOT}\\{script} {args}'
$p = New-ScheduledTaskPrincipal -UserId '{user}' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName '{task}' -Action $a -Principal $p -Force | Out-Null""")


def virsh(*args, check=True):
    # virsh answers in the user's language ("シャットオフ" for "shut off"); the
    # states are compared in English
    env = dict(os.environ, LC_ALL="C")
    r = subprocess.run(["virsh", "-c", CONNECT, *args], capture_output=True, text=True, env=env)
    if check and r.returncode != 0:
        raise VmError(r.stderr.strip() or r.stdout.strip())
    return r.stdout.strip()


def running():
    return virsh("domstate", DOMAIN) == "running"


def start_vm():
    virsh("start", DOMAIN)
    print(f"started {DOMAIN}; waiting for Windows", file=sys.stderr)


def wait_for_sign_in(limit=900):
    """Wait for the guest agent, then for a signed-in user."""
    t0, told = time.time(), False
    while time.time() - t0 < limit:
        time.sleep(5)
        try:
            agent("guest-ping")
            user = ps_ok("(Get-CimInstance Win32_ComputerSystem).UserName", 30).strip()
        except (VmError, json.JSONDecodeError, KeyError):
            continue
        if user:
            # Give the desktop a moment after sign-in before Office starts
            time.sleep(20)
            return
        if not told:
            print("sign in to Windows (virt-manager's console); the tool waits", file=sys.stderr)
            told = True
    raise VmError(f"nobody signed in to Windows within {limit}s")


def shut_down(limit=300):
    """Shut Windows down the normal way (ACPI) and wait until the VM is off."""
    virsh("shutdown", DOMAIN)
    t0 = time.time()
    while time.time() - t0 < limit:
        time.sleep(5)
        if virsh("domstate", DOMAIN, check=False) == "shut off":
            print(f"{DOMAIN} is shut down", file=sys.stderr)
            return True
    print(f"{DOMAIN} did not shut down in {limit}s; shut it down in Windows", file=sys.stderr)
    return False


def office_running():
    out = ps_ok("@(Get-Process WINWORD,EXCEL -ErrorAction SilentlyContinue | "
                "ForEach-Object { $_.ProcessName + ' ' + $_.Id }) -join ', '")
    return out.strip()


def run(sources, out_dir, timeout):
    user = ps_ok("(Get-CimInstance Win32_ComputerSystem).UserName").strip()
    if not user:
        raise VmError("nobody is signed in to Windows; sign in first")
    busy = office_running()
    if busy:
        raise VmError(f"Word or Excel is already running ({busy}); close it first")

    ps_ok(f"New-Item -ItemType Directory -Force {ROOT}\\in,{ROOT}\\out | Out-Null; "
          f"Remove-Item -Force {ROOT}\\in\\*,{ROOT}\\out\\*,{ROOT}\\done.txt -ErrorAction SilentlyContinue")
    for name in ("ms_pdf.ps1", "close.ps1"):
        put_script(name)
    for src in sources:
        put(src.read_bytes(), f"{ROOT}\\in\\{src.name}")

    since = ps_ok("Get-Date -Format s").strip()
    register(TASK, "ms_pdf.ps1", f"-Root {ROOT}", user)
    register(TASK_CLOSE, "close.ps1", f"-Root {ROOT} -Since {since}", user)
    status = 0
    try:
        ps_ok(f"Start-ScheduledTask -TaskName '{TASK}'")
        t0 = time.time()
        done = False
        while time.time() - t0 < timeout:
            time.sleep(2)
            if ps_ok(f"Test-Path {ROOT}\\done.txt").strip() == "True":
                done = True
                break
        if not done:
            print(f"Word / Excel did not finish in {timeout}s; closing them", file=sys.stderr)
            ps_ok(f"Start-ScheduledTask -TaskName '{TASK_CLOSE}'; Start-Sleep 15; "
                  f"Get-Content -Encoding UTF8 {ROOT}\\close.txt")
        # Quit returns before the process has gone; give it a moment
        for _ in range(15):
            left = ps_ok("@(Get-Process WINWORD,EXCEL -ErrorAction SilentlyContinue | "
                         f"Where-Object {{ $_.StartTime -ge [datetime]::Parse('{since}') }} | "
                         "ForEach-Object { $_.ProcessName + ' ' + $_.Id }) -join ', '").strip()
            if not left:
                break
            time.sleep(2)
        if left:
            print(f"STILL RUNNING in Windows: {left}. Not killed; close it there "
                  "(a dialog may be waiting for an answer)", file=sys.stderr)
            status = 2
        print(ps_ok(f"Get-Content -Encoding UTF8 {ROOT}\\log.txt"), end="")
    finally:
        if status != 2:
            ps(f"Stop-ScheduledTask -TaskName '{TASK}' -ErrorAction SilentlyContinue")
        ps(f"Unregister-ScheduledTask -TaskName '{TASK}' -Confirm:$false -ErrorAction SilentlyContinue; "
           f"Unregister-ScheduledTask -TaskName '{TASK_CLOSE}' -Confirm:$false -ErrorAction SilentlyContinue")

    for src in sources:
        dst = (out_dir or src.parent) / (src.stem + ".ms.pdf")
        dst.unlink(missing_ok=True)
        try:
            dst.write_bytes(get(f"{ROOT}\\out\\{src.stem}.ms.pdf"))
            print(f"→ {dst}")
        except VmError:
            print(f"× {src.name}: no PDF", file=sys.stderr)
            status = status or 1
    return status


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("files", nargs="+", type=Path)
    ap.add_argument("--out", type=Path, help="folder for the PDFs (default: next to each file)")
    ap.add_argument("--timeout", type=int, default=300, help="seconds for the whole run")
    ap.add_argument("--keep", action="store_true", help="leave the VM running if the tool started it")
    a = ap.parse_args()
    bad = [f for f in a.files if f.suffix.lower() not in (".docx", ".xlsx") or not f.is_file()]
    if bad:
        ap.error("not a docx / xlsx file: " + ", ".join(map(str, bad)))
    names = [f.name for f in a.files]
    if len(set(names)) != len(names):
        ap.error("two files have the same name")
    if a.out:
        a.out.mkdir(parents=True, exist_ok=True)
    started = False
    status = 2
    try:
        if not running():
            start_vm()
            # Noted before waiting, so a VM nobody signed in to is shut down too
            started = True
            wait_for_sign_in()
        status = run([f.resolve() for f in a.files], a.out, a.timeout)
    except VmError as e:
        print(f"winvm_pdf: {e}", file=sys.stderr)
    finally:
        # Shut down only a VM this run started, and never with Word or Excel
        # of this run still open (status 2 then says so)
        if started and not a.keep:
            try:
                if not office_running():
                    shut_down()
                else:
                    print("Word or Excel is still running; the VM is left on", file=sys.stderr)
            except VmError as e:
                print(f"winvm_pdf: {e}", file=sys.stderr)
    sys.exit(status)


if __name__ == "__main__":
    main()
