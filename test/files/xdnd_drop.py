#!/usr/bin/env python3
"""A minimal XDND source (no xdotool, no GTK here): drop FILE onto the window under (X, Y) as a file manager does.
usage: xdnd_drop.py X Y FILE...    Uses $DISPLAY. Prints "dropped" when the target asked for the data and said it finished, else the step it stopped at (exit 1).
Protocol (XDND 5): own XdndSelection, XdndEnter + XdndPosition to the toplevel that has XdndAware, wait for XdndStatus, XdndDrop, answer the
target's selection request with text/uri-list, wait for XdndFinished."""
import ctypes, ctypes.util, os, struct, sys, time
x11 = ctypes.CDLL(ctypes.util.find_library('X11'))
V = ctypes.c_void_p; UL = ctypes.c_ulong
x11.XOpenDisplay.restype = V; x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XInternAtom.restype = UL; x11.XInternAtom.argtypes = [V, ctypes.c_char_p, ctypes.c_int]
x11.XDefaultRootWindow.restype = UL; x11.XDefaultRootWindow.argtypes = [V]
x11.XCreateSimpleWindow.restype = UL; x11.XCreateSimpleWindow.argtypes = [V, UL, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint, ctypes.c_uint, UL, UL]
x11.XSetSelectionOwner.argtypes = [V, UL, UL, UL]
x11.XQueryPointer.argtypes = [V, UL, ctypes.POINTER(UL), ctypes.POINTER(UL), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_uint)]
x11.XGetWindowProperty.argtypes = [V, UL, UL, ctypes.c_long, ctypes.c_long, ctypes.c_int, UL, ctypes.POINTER(UL), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(UL), ctypes.POINTER(UL), ctypes.POINTER(ctypes.c_void_p)]
x11.XSendEvent.argtypes = [V, UL, ctypes.c_int, ctypes.c_long, V]; x11.XFlush.argtypes = [V]; x11.XPending.argtypes = [V]; x11.XNextEvent.argtypes = [V, V]
x11.XChangeProperty.argtypes = [V, UL, UL, UL, ctypes.c_int, ctypes.c_int, ctypes.c_char_p, ctypes.c_int]
x11.XWarpPointer.argtypes = [V, UL, UL, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint, ctypes.c_int, ctypes.c_int]
x11.XSync.argtypes = [V, ctypes.c_int]
x, y = int(sys.argv[1]), int(sys.argv[2]); files = [os.path.abspath(f) for f in sys.argv[3:]]
d = x11.XOpenDisplay(os.environ.get('DISPLAY', ':0').encode())
if not d: sys.exit('cannot open display')
A = lambda n: x11.XInternAtom(d, n.encode(), 0)
root = x11.XDefaultRootWindow(d); me = x11.XCreateSimpleWindow(d, root, 0, 0, 1, 1, 0, 0, 0)
sel, uri, plain = A('XdndSelection'), A('text/uri-list'), A('XdndAware')
def aware(w):
    t, f, n, a, p = UL(), ctypes.c_int(), UL(), UL(), ctypes.c_void_p()
    x11.XGetWindowProperty(d, w, plain, 0, 1, 0, 0, ctypes.byref(t), ctypes.byref(f), ctypes.byref(n), ctypes.byref(a), ctypes.byref(p)); return n.value > 0
x11.XQueryTree.argtypes = [V, UL, ctypes.POINTER(UL), ctypes.POINTER(UL), ctypes.POINTER(ctypes.POINTER(UL)), ctypes.POINTER(ctypes.c_uint)]
def kids(w):
    r, p, ch, n = UL(), UL(), ctypes.POINTER(UL)(), ctypes.c_uint()
    if not x11.XQueryTree(d, w, ctypes.byref(r), ctypes.byref(p), ctypes.byref(ch), ctypes.byref(n)): return []
    return [ch[i] for i in range(n.value)]
def target():
    # the window manager reparents the app's window into frames: search the whole tree for the XdndAware window (a display here holds one app)
    stack = [root]
    while stack:
        w = stack.pop()
        if aware(w): return w
        stack.extend(kids(w))
    return None
x11.XWarpPointer(d, 0, root, 0, 0, 0, 0, x, y); x11.XSync(d, 0); time.sleep(0.2)
tw = target()
if not tw: sys.exit('no XdndAware window under the pointer')
x11.XSetSelectionOwner(d, sel, me, 0)
def msg(mtype, l):
    ev = ctypes.create_string_buffer(192); struct.pack_into('i', ev, 0, 33); struct.pack_into('L', ev, 32, tw); struct.pack_into('L', ev, 40, A(mtype)); struct.pack_into('i', ev, 48, 32)
    for i, v in enumerate(l): struct.pack_into('L', ev, 56 + 8 * i, v)
    x11.XSendEvent(d, tw, 0, 0, ev); x11.XFlush(d)
def events(timeout):
    end = time.time() + timeout
    while time.time() < end:
        if x11.XPending(d):
            ev = ctypes.create_string_buffer(192); x11.XNextEvent(d, ev); yield ev
        else: time.sleep(0.02)
served = False
def serve(ev):
    """Answer the target's request for the dragged data (GPUI asks while the drag moves, at XdndPosition, not only at the drop)."""
    global served
    if struct.unpack_from('i', ev, 0)[0] != 30: return
    req, selection, target_, prop = [struct.unpack_from('L', ev, o)[0] for o in (40, 48, 56, 64)]
    body = ''.join('file://' + f + '\r\n' for f in files).encode()
    if target_ == uri: x11.XChangeProperty(d, req, prop, uri, 8, 0, body, len(body))
    out = ctypes.create_string_buffer(192); struct.pack_into('i', out, 0, 31); struct.pack_into('L', out, 32, req); struct.pack_into('L', out, 40, selection); struct.pack_into('L', out, 48, target_)
    struct.pack_into('L', out, 56, prop if target_ == uri else 0); struct.pack_into('L', out, 64, 0); x11.XSendEvent(d, req, 0, 0, out); x11.XFlush(d); served = True
msg('XdndEnter', [me, 5 << 24, uri, 0, 0]); time.sleep(0.1)
accepted = False
msg('XdndPosition', [me, 0, (x << 16) | y, 0, A('XdndActionCopy')])
for ev in events(3):
    serve(ev); t = struct.unpack_from('i', ev, 0)[0]
    if t == 33 and struct.unpack_from('L', ev, 40)[0] == A('XdndStatus'): accepted = bool(struct.unpack_from('L', ev, 64)[0] & 1); break
if not accepted: msg('XdndLeave', [me, 0, 0, 0, 0]); sys.exit('the target did not accept the drop (no XdndStatus accept)')
for ev in events(0.6): serve(ev)   # the request may follow the status
if not served: sys.exit('the target never asked for the data')
msg('XdndDrop', [me, 0, 0, 0, 0])
for ev in events(3):
    serve(ev)
    if struct.unpack_from('i', ev, 0)[0] == 33 and struct.unpack_from('L', ev, 40)[0] == A('XdndFinished'): print('dropped (finished)'); sys.exit(0)
print('dropped (no XdndFinished seen)'); sys.exit(0)
