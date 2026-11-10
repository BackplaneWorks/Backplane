#!/usr/bin/env python3
"""Real X input through XTest (no xdotool here): xinput.py move X Y | click X Y [button] | drag X1 Y1 X2 Y2 | type TEXT | key KEYSYM-NAME | chord MOD... KEYSYM-NAME | down KEYSYM-NAME | up KEYSYM-NAME | focus TITLE | pos
(`down`/`up`: a key held across commands, e.g. Control_L held while a screenshot is taken. `focus`: give the keyboard to the window whose title starts with
TITLE, as a window manager does on a click: without a FocusIn, GPUI's X11 client sees no modifier changes and thinks its window inactive.)
Uses $DISPLAY. Events go through the X server, so they reach the GPUI window exactly like a person's mouse and keyboard."""
import ctypes, ctypes.util, os, sys, time
x11 = ctypes.CDLL(ctypes.util.find_library('X11')); xt = ctypes.CDLL(ctypes.util.find_library('Xtst'))
x11.XOpenDisplay.restype = ctypes.c_void_p; x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XStringToKeysym.restype = ctypes.c_ulong; x11.XStringToKeysym.argtypes = [ctypes.c_char_p]
x11.XKeysymToKeycode.restype = ctypes.c_ubyte; x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
x11.XFlush.argtypes = [ctypes.c_void_p]; x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
xt.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
xt.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
xt.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
d = x11.XOpenDisplay(os.environ.get('DISPLAY', ':0').encode())
if not d: sys.exit('cannot open display')
def move(x, y): xt.XTestFakeMotionEvent(d, -1, int(x), int(y), 0); x11.XSync(d, 0); time.sleep(0.05)
def click(b=1): xt.XTestFakeButtonEvent(d, b, 1, 0); x11.XSync(d, 0); time.sleep(0.06); xt.XTestFakeButtonEvent(d, b, 0, 0); x11.XSync(d, 0); time.sleep(0.05)
def key(name, shift=False):
    ks = x11.XStringToKeysym(name.encode()); kc = x11.XKeysymToKeycode(d, ks)
    if not kc: sys.exit('no keycode for ' + name)
    sk = x11.XKeysymToKeycode(d, x11.XStringToKeysym(b'Shift_L'))
    if shift: xt.XTestFakeKeyEvent(d, sk, 1, 0)
    xt.XTestFakeKeyEvent(d, kc, 1, 0); x11.XSync(d, 0); time.sleep(0.03); xt.XTestFakeKeyEvent(d, kc, 0, 0)
    if shift: xt.XTestFakeKeyEvent(d, sk, 0, 0)
    x11.XSync(d, 0); time.sleep(0.04)
NAMES = {' ': 'space', '.': 'period', ',': 'comma', '/': 'slash', ':': 'colon', '-': 'minus', '_': 'underscore', '@': 'at', '&': 'ampersand', '>': 'greater', '%': 'percent', '$': 'dollar', '#': 'numbersign', '~': 'asciitilde', '^': 'asciicircum'}
def chord(names):
    held = [x11.XKeysymToKeycode(d, x11.XStringToKeysym({'ctrl': b'Control_L', 'shift': b'Shift_L', 'alt': b'Alt_L', 'super': b'Super_L'}[m])) for m in names[:-1]]
    for k in held: xt.XTestFakeKeyEvent(d, k, 1, 0)
    x11.XSync(d, 0); time.sleep(0.03); key(names[-1]); time.sleep(0.03)
    for k in reversed(held): xt.XTestFakeKeyEvent(d, k, 0, 0)
    x11.XSync(d, 0); time.sleep(0.04)
cmd, a = sys.argv[1], sys.argv[2:]
if cmd == 'move': move(a[0], a[1])
elif cmd == 'click': move(a[0], a[1]); click(int(a[2]) if len(a) > 2 else 1)
elif cmd == 'type':
    for ch in a[0]:
        name = NAMES.get(ch, ch); key(name, shift=ch.isupper() or ch in ':_@&>%$#~^')
elif cmd == 'key': key(a[0])
elif cmd == 'drag':  # drag X1 Y1 X2 Y2: press, move in steps (a drag starts past a few pixels), release
    x1, y1, x2, y2 = map(float, a[:4]); move(x1, y1); xt.XTestFakeButtonEvent(d, 1, 1, 0); x11.XSync(d, 0); time.sleep(0.1)
    for i in range(1, 13): move(x1 + (x2 - x1) * i / 12, y1 + (y2 - y1) * i / 12); time.sleep(0.03)
    time.sleep(0.15); xt.XTestFakeButtonEvent(d, 1, 0, 0); x11.XSync(d, 0); time.sleep(0.1)
elif cmd == 'chord': chord(a)
elif cmd == 'focus':
    W = ctypes.c_ulong; x11.XDefaultRootWindow.restype = W; x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    x11.XQueryTree.argtypes = [ctypes.c_void_p, W, ctypes.POINTER(W), ctypes.POINTER(W), ctypes.POINTER(ctypes.POINTER(W)), ctypes.POINTER(ctypes.c_uint)]
    x11.XFetchName.argtypes = [ctypes.c_void_p, W, ctypes.POINTER(ctypes.c_char_p)]; x11.XSetInputFocus.argtypes = [ctypes.c_void_p, W, ctypes.c_int, ctypes.c_ulong]
    def find(w):
        name = ctypes.c_char_p()
        if x11.XFetchName(d, w, ctypes.byref(name)) and name.value and name.value.decode(errors='replace').startswith(a[0]): return w
        root, parent, kids, n = W(), W(), ctypes.POINTER(W)(), ctypes.c_uint()
        if not x11.XQueryTree(d, w, ctypes.byref(root), ctypes.byref(parent), ctypes.byref(kids), ctypes.byref(n)): return None
        for i in range(n.value):
            r = find(kids[i])
            if r: return r
        return None
    w = find(x11.XDefaultRootWindow(d))
    if not w: sys.exit('no window titled ' + a[0])
    x11.XSetInputFocus(d, w, 2, 0); x11.XSync(d, 0); time.sleep(0.1)
elif cmd in ('down', 'up'):
    kc = x11.XKeysymToKeycode(d, x11.XStringToKeysym(a[0].encode()))
    if not kc: sys.exit('no keycode for ' + a[0])
    xt.XTestFakeKeyEvent(d, kc, 1 if cmd == 'down' else 0, 0); x11.XSync(d, 0); time.sleep(0.05)
elif cmd == 'drag':  # drag X0 Y0 X1 Y1: the left button down at the first point, moved in steps, up at the second
    x0, y0, x1, y1 = [float(v) for v in a[:4]]; move(x0, y0); xt.XTestFakeButtonEvent(d, 1, 1, 0); x11.XSync(d, 0); time.sleep(0.08)
    for i in range(1, 11): move(x0 + (x1 - x0) * i / 10, y0 + (y1 - y0) * i / 10)
    xt.XTestFakeButtonEvent(d, 1, 0, 0); x11.XSync(d, 0); time.sleep(0.05)
elif cmd == 'dblclick': move(a[0], a[1]); click(1); click(1)
elif cmd == 'wheel': move(a[0], a[1]); click(4 if int(a[2]) > 0 else 5)
x11.XFlush(d)
