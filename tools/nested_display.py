"""A display nobody sees, for looking at officework without touching the desktop.

    python3 -I tools/nested_display.py start :5 [1100x1000]   # runs until stopped
    python3 -I tools/nested_display.py shot :5 out.png         # the officework window as PNG

`start` makes a window on the desktop's X display ($DISPLAY) that is never
mapped, and runs Xephyr inside it. The nested display shows nothing on the
desktop and never takes the keyboard, so the app cannot catch what the
person at the desk is typing (2026-10-15). Stop it with Ctrl+C or SIGTERM;
Xephyr stops with it.

Run the app on the nested display with

    env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY=:5 \\
      MESA_VK_WSI_DEBUG=sw HOME=<work home> XDG_RUNTIME_DIR=<work dir> \\
      ./target/release/officework <copy of a file>

Xephyr has no DRI3, so Vulkan presents through Mesa's software path
(MESA_VK_WSI_DEBUG=sw); without it the window stays black. A GPUI window
there draws its first frame only when asked to redraw, so `shot` runs
xrefresh before it captures. Standard library only (libX11 through ctypes).
"""
import ctypes
import signal
import struct
import subprocess
import sys
import time
import zlib


def start(display, size):
    w, h = (int(v) for v in size.split("x"))
    x11 = ctypes.CDLL("libX11.so.6")
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XDefaultScreen.argtypes = [ctypes.c_void_p]
    x11.XRootWindow.restype = ctypes.c_ulong
    x11.XRootWindow.argtypes = [ctypes.c_void_p, ctypes.c_int]
    x11.XCreateSimpleWindow.restype = ctypes.c_ulong
    x11.XCreateSimpleWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int,
                                        ctypes.c_uint, ctypes.c_uint, ctypes.c_uint,
                                        ctypes.c_ulong, ctypes.c_ulong]
    x11.XFlush.argtypes = [ctypes.c_void_p]
    dpy = x11.XOpenDisplay(None)
    if not dpy:
        raise SystemExit("nested_display: cannot open $DISPLAY")
    root = x11.XRootWindow(dpy, x11.XDefaultScreen(dpy))
    # Never mapped: nothing appears on the desktop and it can never have focus
    host = x11.XCreateSimpleWindow(dpy, root, 0, 0, w, h, 0, 0, 0)
    x11.XFlush(dpy)
    xephyr = subprocess.Popen(["Xephyr", display, "-parent", hex(host), "-screen", size,
                               "-ac", "-noreset", "-nolisten", "tcp"])
    print(f"{display} is ready ({size}); Ctrl+C stops it", file=sys.stderr, flush=True)

    def stop(*_):
        xephyr.terminate()
        xephyr.wait(timeout=10)
        sys.exit(0)

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    while xephyr.poll() is None:
        time.sleep(1)
    sys.exit(xephyr.returncode)


def window_of(display):
    tree = subprocess.run(["xwininfo", "-display", display, "-root", "-tree"],
                          capture_output=True, text=True, check=True).stdout
    for line in tree.splitlines():
        parts = line.split()
        if len(parts) > 1 and parts[1] == '"officework":':
            return parts[0]
    raise SystemExit(f"nested_display: no officework window on {display}")


def xwd_to_png(data, out):
    hdr = struct.unpack(">25I", data[:100])
    header_size, fmt, w, h = hdr[0], hdr[2], hdr[4], hdr[5]
    byte_order, bpp, bpl, ncolors = hdr[7], hdr[11], hdr[12], hdr[19]
    if fmt != 2 or bpp not in (24, 32):
        raise SystemExit(f"nested_display: unknown xwd format {fmt}/{bpp}")
    step, off = bpp // 8, header_size + ncolors * 12
    rows = []
    for y in range(h):
        line = data[off + y * bpl: off + y * bpl + w * step]
        row = bytearray(b"\x00")
        for i in range(0, len(line), step):
            px = line[i:i + step]
            # LSB first is B G R (X); MSB first is (X) R G B
            row += bytes((px[2], px[1], px[0])) if byte_order == 0 else px[-3:]
        rows.append(bytes(row))

    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(b"".join(rows), 6)) + chunk(b"IEND", b""))
    with open(out, "wb") as f:
        f.write(png)


def shot(display, out):
    subprocess.run(["xrefresh", "-display", display], check=True)
    time.sleep(1.5)
    win = window_of(display)
    data = subprocess.run(["xwd", "-silent", "-display", display, "-id", win],
                          capture_output=True, check=True).stdout
    xwd_to_png(data, out)
    print(out)


def main():
    if len(sys.argv) >= 3 and sys.argv[1] == "start":
        start(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else "1100x1000")
    elif len(sys.argv) == 4 and sys.argv[1] == "shot":
        shot(sys.argv[2], sys.argv[3])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
