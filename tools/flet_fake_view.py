#!/usr/bin/env python3
"""A fake Flet client, to see the real messages (docs/sekkei/flet-client.ja.adoc).

Put this file as `flet` in a folder, make it executable, and run a Flet
program with FLET_VIEW_PATH pointing at that folder:

    mkdir -p /tmp/view && cp tools/flet_fake_view.py /tmp/view/flet && chmod +x /tmp/view/flet
    FLET_VIEW_PATH=/tmp/view FAKE_LOG=/tmp/view/log.txt python app.py

Flet starts this instead of its Flutter client with `flet <url> <pid file>`.
It connects, registers, logs every message, sends one click to the first
Button it sees, logs the patch that comes back, and exits after 3 s of
silence. The log is the material for the design document (2026-10-17).
Needs msgpack (the flet package depends on it).
"""
import json, os, socket, struct, sys, time
import msgpack

url, pid_file = sys.argv[1], sys.argv[2]
open(pid_file, "w").write(str(os.getpid()))
log = open(os.environ["FAKE_LOG"], "w")
def out(*a):
    print(*a, file=log, flush=True)

def ext_hook(code, data):
    return f"<ext{code}:{data.decode()}>"

if url.startswith("tcp://"):
    host, port = url[6:].rsplit(":", 1)
    s = socket.create_connection((host, int(port)))
else:
    s = socket.socket(socket.AF_UNIX); s.connect(url)
out("connected", url)

def send(action, body):
    packet = b"\x00" + msgpack.packb([action, body], use_bin_type=True)
    s.sendall(struct.pack("<I", len(packet)) + packet)

buf = b""
def recv():
    global buf
    while True:
        if len(buf) >= 4:
            n = struct.unpack("<I", buf[:4])[0]
            if len(buf) >= 4 + n:
                packet, buf = buf[4:4+n], buf[4+n:]
                if packet[0] == 0:
                    return msgpack.unpackb(packet[1:], ext_hook=ext_hook, raw=False, strict_map_key=False)
                out("data channel frame", len(packet))
                continue
        chunk = s.recv(65536)
        if not chunk: return None
        buf += chunk

send(1, {"session_id": None, "page_name": "", "page": {
    "route": "/", "pwa": False, "web": False, "debug": False, "wasm": False, "test": False,
    "multi_view": False, "pyodide": False, "platform_brightness": "light",
    "width": 800, "height": 600, "platform": "linux",
    "window": {"_c": "Window", "_i": 0, "width": 800, "height": 600},
    "media": {"padding": {"top": 0, "right": 0, "bottom": 0, "left": 0},
              "view_padding": {"top": 0, "right": 0, "bottom": 0, "left": 0},
              "view_insets": {"top": 0, "right": 0, "bottom": 0, "left": 0}}}})
m = recv()
out("REGISTER response:", json.dumps(m, ensure_ascii=False, default=str, indent=1))
# find the Button id in the page tree
def find(node, ctype):
    if isinstance(node, dict):
        if node.get("_c") == ctype: return node["_i"]
        for v in node.values():
            r = find(v, ctype)
            if r is not None: return r
    if isinstance(node, list):
        for v in node:
            r = find(v, ctype)
            if r is not None: return r
btn = None
s.settimeout(3)
try:
    while True:
        m = recv()
        if m is None: break
        out("MESSAGE:", json.dumps(m, ensure_ascii=False, default=str))
        if btn is None:
            btn = find(m, "Button")
            if btn is not None:
                out("button id", btn, "-> sending click")
                send(3, {"target": btn, "name": "click", "data": None})
except Exception as e:
    out("end", repr(e))
