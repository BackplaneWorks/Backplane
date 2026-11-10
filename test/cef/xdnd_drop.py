#!/usr/bin/env python3
"""xdnd_drop.py FILE X Y [TITLE]: drop FILE on the window titled TITLE (default "Backplane") at root X,Y, as the Xwayland bridges of Wayland
compositors (Hyprland) do: the X pointer stays where it is, the type list has more than three types, the drop comes right after the first position
(DELAY=secs: after the data was read instead). Prints what the target answered. Uses $DISPLAY (a test display only). Pairs with xinput.py."""
import ctypes, ctypes.util, os, sys, time
X = ctypes.CDLL(ctypes.util.find_library("X11")); W = ctypes.c_ulong
X.XOpenDisplay.restype = ctypes.c_void_p; X.XOpenDisplay.argtypes = [ctypes.c_char_p]
X.XDefaultRootWindow.restype = W; X.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
X.XInternAtom.restype = W; X.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
X.XCreateSimpleWindow.restype = W; X.XCreateSimpleWindow.argtypes = [ctypes.c_void_p, W, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint, ctypes.c_uint, W, W]
X.XSetSelectionOwner.argtypes = [ctypes.c_void_p, W, W, W]
X.XChangeProperty.argtypes = [ctypes.c_void_p, W, W, W, ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
X.XSendEvent.argtypes = [ctypes.c_void_p, W, ctypes.c_int, ctypes.c_long, ctypes.c_void_p]
X.XFlush.argtypes = [ctypes.c_void_p]; X.XPending.argtypes = [ctypes.c_void_p]; X.XNextEvent.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
X.XQueryTree.argtypes = [ctypes.c_void_p, W, ctypes.POINTER(W), ctypes.POINTER(W), ctypes.POINTER(ctypes.POINTER(W)), ctypes.POINTER(ctypes.c_uint)]
X.XFetchName.argtypes = [ctypes.c_void_p, W, ctypes.POINTER(ctypes.c_char_p)]
class CM(ctypes.Structure): _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int), ("display", ctypes.c_void_p), ("window", W), ("message_type", W), ("format", ctypes.c_int), ("l", ctypes.c_long * 5)]
class SR(ctypes.Structure): _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int), ("display", ctypes.c_void_p), ("owner", W), ("requestor", W), ("selection", W), ("target", W), ("property", W), ("time", ctypes.c_ulong)]
class SN(ctypes.Structure): _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int), ("display", ctypes.c_void_p), ("requestor", W), ("selection", W), ("target", W), ("property", W), ("time", ctypes.c_ulong)]
f, x, y = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]); title = sys.argv[4] if len(sys.argv) > 4 else "Backplane"
d = X.XOpenDisplay(os.environ["DISPLAY"].encode()); root = X.XDefaultRootWindow(d)
A = lambda n: X.XInternAtom(d, n.encode(), 0)
def find(w):
    nm = ctypes.c_char_p()
    if X.XFetchName(d, w, ctypes.byref(nm)) and nm.value and nm.value.decode(errors="replace").startswith(title): return w
    r, p, k, n = W(), W(), ctypes.POINTER(W)(), ctypes.c_uint()
    if not X.XQueryTree(d, w, ctypes.byref(r), ctypes.byref(p), ctypes.byref(k), ctypes.byref(n)): return 0
    for i in range(n.value):
        z = find(k[i])
        if z: return z
    return 0
tgt = find(root); assert tgt, "no target window"
src = X.XCreateSimpleWindow(d, root, 0, 0, 1, 1, 0, 0, 0)
sel = A("XdndSelection"); X.XSetSelectionOwner(d, sel, src, 0)
types = [A("application/vnd.portal.filetransfer"), A("application/vnd.portal.files"), A("text/uri-list"), A("text/plain;charset=utf-8")]
arr = (W * len(types))(*types); X.XChangeProperty(d, src, A("XdndTypeList"), A("ATOM"), 32, 0, arr, len(types))
def send(kind, l):
    e = CM(33, 0, 1, d, tgt, A(kind), 32, (ctypes.c_long * 5)(*l)); X.XSendEvent(d, tgt, 0, 0, ctypes.byref(e)); X.XFlush(d)
send("XdndEnter", [src, (5 << 24) | 1, 0, 0, 0]); time.sleep(0.1)
send("XdndPosition", [src, 0, (x << 16) | y, 0, A("XdndActionCopy")])
uri = ("file://" + f + "\r\n").encode(); buf = (ctypes.c_char * 192)(); end = time.time() + 5; got = []
DELAY = float(os.environ.get("DELAY", "0")); dropped = DELAY == 0
if dropped: send("XdndDrop", [src, 0, 0, 0, 0])   # at once: the target has not read the data yet
t0 = time.time()
while time.time() < end:
    while X.XPending(d):
        X.XNextEvent(d, buf); t = ctypes.c_int.from_buffer(buf).value
        if t == 30:  # SelectionRequest
            r = SR.from_buffer(buf); data = ctypes.create_string_buffer(uri, len(uri))
            X.XChangeProperty(d, r.requestor, r.property, r.target, 8, 0, data, len(uri))
            n = SN(31, 0, 1, d, r.requestor, r.selection, r.target, r.property, r.time); X.XSendEvent(d, r.requestor, 0, 0, ctypes.byref(n)); X.XFlush(d); got.append("request")
        elif t == 33:
            c = CM.from_buffer(buf)
            for k in ("XdndStatus", "XdndFinished"):
                if c.message_type == A(k): got.append(k + (" accepted" if k == "XdndFinished" and c.l[1] & 1 else ""))
    if not dropped and time.time() - t0 > DELAY: send("XdndDrop", [src, 0, 0, 0, 0]); dropped = True
    if "XdndFinished" in " ".join(got): break
    time.sleep(0.02)
print(got)
