#!/usr/bin/env python3
"""Continuous input through XTest with its timing, for frame-time runs; prints {"t0": epoch ms, "t1": epoch ms, "events": n}.
  rt_input.py pan CX CY RADIUS SECONDS [BUTTON=2] [HZ=120]   the button held while the pointer circles (KiCad pans with the middle button)
  rt_input.py wheel CX CY SECONDS [HZ=30]                     wheel notches, 8 in then 8 out, repeated (KiCad zooms about the pointer)
  rt_input.py drag X1 Y1 X2 Y2 [MS=400] [BUTTON=1]            press, move in 60 Hz steps, release (a move of a footprint, a box)
  rt_input.py key NAME [MODS...]                               one key (with modifiers held), timed
  rt_input.py click X Y [BUTTON=1]                             one click, timed"""
import ctypes, ctypes.util, json, math, os, sys, time
x11 = ctypes.CDLL(ctypes.util.find_library("X11")); xt = ctypes.CDLL(ctypes.util.find_library("Xtst"))
x11.XOpenDisplay.restype = ctypes.c_void_p; x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XStringToKeysym.restype = ctypes.c_ulong; x11.XStringToKeysym.argtypes = [ctypes.c_char_p]
x11.XKeysymToKeycode.restype = ctypes.c_ubyte; x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
xt.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
xt.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
xt.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
d = x11.XOpenDisplay(os.environ.get("DISPLAY", ":0").encode())
if not d: sys.exit("cannot open display")
now = lambda: round(time.time() * 1000, 1)
def move(x, y): xt.XTestFakeMotionEvent(d, -1, int(x), int(y), 0); x11.XSync(d, 0)
def button(b, down):
    if b <= 0: return  # 0: no button (the pointer only moves)
    xt.XTestFakeButtonEvent(d, b, 1 if down else 0, 0); x11.XSync(d, 0)
def kc(name): return x11.XKeysymToKeycode(d, x11.XStringToKeysym(name.encode()))
MODS = {"ctrl": "Control_L", "shift": "Shift_L", "alt": "Alt_L"}
def paced(seconds, hz, step):
    t_start = time.time(); n = 0
    while time.time() - t_start < seconds:
        step(n); n += 1
        nxt = t_start + n / hz
        if nxt > time.time(): time.sleep(nxt - time.time())
    return n

cmd, a = sys.argv[1], sys.argv[2:]
res = {}
if cmd == "pan":
    cx, cy, r, secs = float(a[0]), float(a[1]), float(a[2]), float(a[3]); b = int(a[4]) if len(a) > 4 else 2; hz = float(a[5]) if len(a) > 5 else 120
    move(cx + r, cy); time.sleep(0.05); res["t0"] = now(); button(b, True)
    n = paced(secs, hz, lambda i: move(cx + r * math.cos(i / hz * 2.5), cy + r * math.sin(i / hz * 2.5)))
    button(b, False); res.update(t1=now(), events=n)
elif cmd == "wheel":
    cx, cy, secs = float(a[0]), float(a[1]), float(a[2]); hz = float(a[3]) if len(a) > 3 else 30
    move(cx, cy); time.sleep(0.05); res["t0"] = now()
    def notch(i): b = 4 if (i // 8) % 2 == 0 else 5; button(b, True); button(b, False)
    n = paced(secs, hz, notch); res.update(t1=now(), events=n)
elif cmd == "drag":
    x1, y1, x2, y2 = map(float, a[:4]); ms = float(a[4]) if len(a) > 4 else 400; b = int(a[5]) if len(a) > 5 else 1
    move(x1, y1); time.sleep(0.05); res["t0"] = now(); button(b, True); time.sleep(0.05)
    steps = max(2, int(ms / 16.7)); n = paced(ms / 1000, 60, lambda i: move(x1 + (x2 - x1) * min(1, (i + 1) / steps), y1 + (y2 - y1) * min(1, (i + 1) / steps)))
    move(x2, y2); time.sleep(0.03); button(b, False); res.update(t1=now(), events=n)
elif cmd == "key":
    held = [kc(MODS[m]) for m in a[1:]]
    for k in held: xt.XTestFakeKeyEvent(d, k, 1, 0)
    x11.XSync(d, 0); time.sleep(0.03); res["t0"] = now()
    k = kc(a[0]); xt.XTestFakeKeyEvent(d, k, 1, 0); x11.XSync(d, 0); time.sleep(0.03); xt.XTestFakeKeyEvent(d, k, 0, 0)
    for k in reversed(held): xt.XTestFakeKeyEvent(d, k, 0, 0)
    x11.XSync(d, 0); res["t1"] = now()
elif cmd == "click":
    move(float(a[0]), float(a[1])); time.sleep(0.05); b = int(a[2]) if len(a) > 2 else 1
    res["t0"] = now(); button(b, True); time.sleep(0.05); button(b, False); res["t1"] = now()
else:
    sys.exit("unknown " + cmd)
print(json.dumps(res))
