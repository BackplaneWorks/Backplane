"""The window manager's own ways to place Backplane's client window, so the suites run the same under i3 and under mutter (KEMBED_WM=mutter:
GNOME as the person runs it, where a client's own XMoveResizeWindow is only a request and i3-msg does not exist).
  wm_ctl.py moveresize X Y [W H]   the client window there (and that size): _NET_MOVERESIZE_WINDOW, after taking it out of maximized
  wm_ctl.py maximize               the client window maximized (_NET_WM_STATE), as i3 tiles it over the whole screen
  wm_ctl.py keep-maximized SECONDS each client window maximized as it appears, for that long (the suites start their clients themselves)
  wm_ctl.py activate XID           that window given the focus as mutter's click-to-focus would (an XTest click never reaches mutter)
  wm_ctl.py screenshot FILE        the screen, pieced from the windows' own pictures (mutter's rootless Xwayland has no root picture)
The client window is the one titled "Backplane GPUI" in the window manager's _NET_CLIENT_LIST."""
import ctypes, ctypes.util, json, os, sys, time

X = ctypes.CDLL(ctypes.util.find_library("X11"))
X.XOpenDisplay.restype = ctypes.c_void_p
X.XDefaultRootWindow.restype = ctypes.c_ulong
X.XInternAtom.restype = ctypes.c_ulong
X.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
X.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
U, P = ctypes.c_ulong, ctypes.c_void_p
X.XGetWindowProperty.argtypes = [P, U, U, ctypes.c_long, ctypes.c_long, ctypes.c_int, U, P, P, P, P, P]
X.XQueryTree.argtypes = [P, U, P, P, P, P]
X.XFetchName.argtypes = [P, U, P]
X.XSendEvent.argtypes = [P, U, ctypes.c_int, ctypes.c_long, P]
X.XFree.argtypes = [P]; X.XFlush.argtypes = [P]
X.XGetWindowAttributes.argtypes = [P, U, P]; X.XDisplayWidth.argtypes = [P, ctypes.c_int]; X.XDisplayHeight.argtypes = [P, ctypes.c_int]
D = ctypes.c_void_p(X.XOpenDisplay(os.environ["DISPLAY"].encode()))
if not D.value: print('{"ok": false, "error": "cannot open the display"}'); sys.exit(2)
ROOT = X.XDefaultRootWindow(D)
def atom(n): return X.XInternAtom(D, n.encode(), 0)


class Data(ctypes.Union): _fields_ = [("l", ctypes.c_long * 5)]
class ClientMessage(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int), ("display", ctypes.c_void_p),
                ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong), ("format", ctypes.c_int), ("data", Data)]
class XEvent(ctypes.Union): _fields_ = [("xclient", ClientMessage), ("pad", ctypes.c_long * 24)]


def prop_windows(w, name):
    t, f, n, left = ctypes.c_ulong(), ctypes.c_int(), ctypes.c_ulong(), ctypes.c_ulong(); data = ctypes.POINTER(ctypes.c_ulong)()
    if X.XGetWindowProperty(D, ctypes.c_ulong(w), ctypes.c_ulong(atom(name)), 0, 4096, 0, 0, ctypes.byref(t), ctypes.byref(f), ctypes.byref(n), ctypes.byref(left), ctypes.byref(data)) != 0 or not data:
        return []
    out = [data[i] for i in range(n.value)]; X.XFree(ctypes.cast(data, P)); return out


def title(w):
    t, f, n, left = ctypes.c_ulong(), ctypes.c_int(), ctypes.c_ulong(), ctypes.c_ulong(); data = ctypes.c_char_p()
    if X.XGetWindowProperty(D, ctypes.c_ulong(w), ctypes.c_ulong(atom("_NET_WM_NAME")), 0, 1024, 0, 0, ctypes.byref(t), ctypes.byref(f), ctypes.byref(n), ctypes.byref(left), ctypes.byref(data)) != 0 or not data.value:
        return ""
    s = data.value.decode("utf-8", "replace"); return s


def wm_name(w):
    name = ctypes.c_char_p()
    if X.XFetchName(D, ctypes.c_ulong(w), ctypes.byref(name)) and name.value: return name.value.decode("utf-8", "replace")
    return ""


def tree(w, depth=0):
    """Every window under w (to 3 levels: mutter's frame, the client, its children)."""
    root, par, kids, n = ctypes.c_ulong(), ctypes.c_ulong(), ctypes.POINTER(ctypes.c_ulong)(), ctypes.c_uint()
    if depth > 2 or not X.XQueryTree(D, ctypes.c_ulong(w), ctypes.byref(root), ctypes.byref(par), ctypes.byref(kids), ctypes.byref(n)): return []
    out = [kids[i] for i in range(n.value)]
    if kids: X.XFree(ctypes.cast(kids, P))
    return out + [x for k in out for x in tree(k, depth + 1)]


def clients():
    """The client's top-level windows: the window manager's list, else any window so titled under the root."""
    named = lambda w: "Backplane GPUI" in (title(w) or wm_name(w))
    ws = [w for w in prop_windows(ROOT, "_NET_CLIENT_LIST") if named(w)]
    return ws or [w for w in tree(ROOT) if named(w) and prop_windows(w, "WM_STATE") != []]


def send(w, msg, *l):
    e = XEvent(); c = e.xclient
    c.type, c.send_event, c.display, c.window, c.message_type, c.format = 33, 1, D, w, atom(msg), 32
    for i, v in enumerate(l): c.data.l[i] = v
    X.XSendEvent(D, ctypes.c_ulong(ROOT), 0, (1 << 19) | (1 << 20), ctypes.byref(e)); X.XFlush(D)


def server_time():
    """The X server's time now (mutter refuses an activation stamped 0): a property appended to a window of ours, and its PropertyNotify's time."""
    X.XCreateSimpleWindow.restype = U; X.XCreateSimpleWindow.argtypes = [P, U, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint, ctypes.c_uint, U, U]
    X.XSelectInput.argtypes = [P, U, ctypes.c_long]; X.XChangeProperty.argtypes = [P, U, U, U, ctypes.c_int, ctypes.c_int, P, ctypes.c_int]
    X.XNextEvent.argtypes = [P, P]
    w = X.XCreateSimpleWindow(D, ROOT, -10, -10, 1, 1, 0, 0, 0)
    X.XSelectInput(D, w, 1 << 22)  # PropertyChangeMask
    a = atom("_BP_TIME"); X.XChangeProperty(D, w, a, atom("STRING"), 8, 2, None, 0); X.XFlush(D)
    class PropertyEvent(ctypes.Structure):
        _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int), ("display", ctypes.c_void_p),
                    ("window", ctypes.c_ulong), ("atom", ctypes.c_ulong), ("time", ctypes.c_ulong), ("state", ctypes.c_int)]
    class Ev(ctypes.Union): _fields_ = [("xproperty", PropertyEvent), ("pad", ctypes.c_long * 24)]
    e = Ev()
    for _ in range(50):
        X.XNextEvent(D, ctypes.byref(e))
        if e.xproperty.type == 28: return e.xproperty.time  # PropertyNotify
    return 0


def maximize(w, on=True):
    send(w, "_NET_WM_STATE", 1 if on else 0, atom("_NET_WM_STATE_MAXIMIZED_VERT"), atom("_NET_WM_STATE_MAXIMIZED_HORZ"), 2, 0)


def moveresize(w, x, y, ww=None, hh=None):
    maximize(w, False); time.sleep(0.2)
    flags = 1 | (1 << 8) | (1 << 9) | ((1 << 10) | (1 << 11) if ww else 0) | (2 << 12)  # NorthWest gravity, x y [w h], from a pager
    send(w, "_NET_MOVERESIZE_WINDOW", flags, x, y, ww or 0, hh or 0)


def screenshot(path):
    """The screen as i3's root picture gives it, under a rootless X server (mutter's Xwayland: its root has no picture): each top-level
    window's own picture, pasted where it is on the screen, the bottom one first."""
    import subprocess, tempfile
    from PIL import Image
    w_, h_ = X.XDisplayWidth(D, 0), X.XDisplayHeight(D, 0)
    canvas = Image.new("RGB", (w_, h_))
    class Attrs(ctypes.Structure):
        _fields_ = [("x", ctypes.c_int), ("y", ctypes.c_int), ("width", ctypes.c_int), ("height", ctypes.c_int), ("border_width", ctypes.c_int),
                    ("depth", ctypes.c_int), ("visual", ctypes.c_void_p), ("root", ctypes.c_ulong), ("c_class", ctypes.c_int), ("bit_gravity", ctypes.c_int),
                    ("win_gravity", ctypes.c_int), ("backing_store", ctypes.c_int), ("backing_planes", ctypes.c_ulong), ("backing_pixel", ctypes.c_ulong),
                    ("save_under", ctypes.c_int), ("colormap", ctypes.c_ulong), ("map_installed", ctypes.c_int), ("map_state", ctypes.c_int),
                    ("all_event_masks", ctypes.c_long), ("your_event_mask", ctypes.c_long), ("do_not_propagate_mask", ctypes.c_long),
                    ("override_redirect", ctypes.c_int), ("screen", ctypes.c_void_p)]
    attrs = Attrs()
    root, par, kids, n = ctypes.c_ulong(), ctypes.c_ulong(), ctypes.POINTER(ctypes.c_ulong)(), ctypes.c_uint()
    X.XQueryTree(D, ctypes.c_ulong(ROOT), ctypes.byref(root), ctypes.byref(par), ctypes.byref(kids), ctypes.byref(n))
    tops = [kids[i] for i in range(n.value)]
    for w in tops:
        if X.XGetWindowAttributes(D, ctypes.c_ulong(w), ctypes.byref(attrs)) == 0: continue
        x, y, ww, hh, map_state = attrs.x, attrs.y, attrs.width, attrs.height, attrs.map_state
        if map_state != 2 or ww < 8 or hh < 8: continue  # IsViewable
        with tempfile.NamedTemporaryFile(suffix=".png") as f:
            if subprocess.run(["import", "-window", str(w), f.name], capture_output=True).returncode != 0: continue
            try: canvas.paste(Image.open(f.name).convert("RGB"), (x, y))
            except Exception: continue
    canvas.save(path)


if __name__ == "__main__":
    if sys.argv[1] == "screenshot": screenshot(sys.argv[2]); sys.exit(0)
    # a window activated as the window manager's own click would (_NET_ACTIVE_WINDOW from a pager, the time now): under mutter an XTest
    # click goes into Xwayland straight, never through the compositor, so its click-to-focus never runs (a person's click does)
    if sys.argv[1] == "close":  # a window closed as its title bar's X would (_NET_CLOSE_WINDOW)
        w = int(sys.argv[2], 0); send(w, "_NET_CLOSE_WINDOW", server_time(), 2, 0, 0, 0); print(json.dumps({"ok": True, "closed": hex(w)})); sys.exit(0)
    if sys.argv[1] == "activate":
        w = int(sys.argv[2], 0); t = server_time()
        send(w, "_NET_ACTIVE_WINDOW", 2, t, 0, 0, 0); print(json.dumps({"ok": True, "activated": hex(w), "time": t})); sys.exit(0)
    cmd, a = sys.argv[1], [int(v) for v in sys.argv[2:]]
    if cmd == "keep-maximized":
        done, t = set(), time.time()
        while time.time() - t < a[0]:
            for w in clients():
                if w not in done: time.sleep(0.3); maximize(w); done.add(w)
            time.sleep(0.2)
        sys.exit(0)
    ws = clients()
    if not ws: print(json.dumps({"ok": False, "error": "no Backplane GPUI client window"})); sys.exit(1)
    for w in ws:
        if cmd == "maximize": maximize(w)
        elif cmd == "moveresize": moveresize(w, *a)
    print(json.dumps({"ok": True, "windows": [hex(w) for w in ws], cmd: a}))
