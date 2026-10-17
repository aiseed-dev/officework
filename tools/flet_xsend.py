"""Sends clicks and keys to the X display :0 with XTEST, to check a Flet
page drawn by officework (needs python-xlib; the weather venv has it).

    flet_xsend.py click X Y        (screen pixels: the window's origin plus the point in it)
    flet_xsend.py type TEXT        (ASCII)
    flet_xsend.py key Return BackSpace Tab ...
"""
import sys, time
from Xlib import X, XK, display
from Xlib.ext import xtest

d = display.Display(":0")

def key(sym):
    code = d.keysym_to_keycode(XK.string_to_keysym(sym))
    xtest.fake_input(d, X.KeyPress, code); d.sync(); time.sleep(0.02)
    xtest.fake_input(d, X.KeyRelease, code); d.sync(); time.sleep(0.05)

def char(c):
    ks = XK.string_to_keysym(c) or XK.string_to_keysym({" ": "space", ".": "period", "-": "minus", "@": "at"}.get(c, c))
    code = d.keysym_to_keycode(ks)
    upper = c.isupper() or c in "@!#$%^&*()_+{}|:\"<>?~"
    sh = d.keysym_to_keycode(XK.XK_Shift_L)
    if upper:
        xtest.fake_input(d, X.KeyPress, sh)
    xtest.fake_input(d, X.KeyPress, code); d.sync(); time.sleep(0.02)
    xtest.fake_input(d, X.KeyRelease, code)
    if upper:
        xtest.fake_input(d, X.KeyRelease, sh)
    d.sync(); time.sleep(0.05)

cmd = sys.argv[1]
if cmd == "click":
    x, y = int(sys.argv[2]), int(sys.argv[3])
    xtest.fake_input(d, X.MotionNotify, x=x, y=y); d.sync(); time.sleep(0.1)
    xtest.fake_input(d, X.ButtonPress, 1); d.sync(); time.sleep(0.05)
    xtest.fake_input(d, X.ButtonRelease, 1); d.sync()
elif cmd == "type":
    for c in sys.argv[2]:
        char(c)
elif cmd == "key":
    for k in sys.argv[2:]:
        key(k)
