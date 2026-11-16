#!/usr/bin/env python3
"""embed_dialog_focus_e2e's drive: KiCad's own dialogs get the keyboard when it was KiCad's, and never take it from the person in Backplane.

  dialog_focus.py OUT          (env: KEMBED_LIB, DRIVER_PY, IPC_PY, PCB; the client's report tagged "r" in OUT/client.log)

Each round first runs what left a dialog without the keyboard (real_use's focus journey, 2 of 5 runs on i3): Backplane's palette (Ctrl+K),
its comment window (Ctrl+;), the palette again, a click back on the canvas. Then a dialog comes up: by a key (a footprint selected, `e`:
Footprint Properties), by a click on a toolbar button (Board Setup), or from inside a dialog (Board Setup's Import Settings from Another Board...).
Each must have the keyboard, Escape must close it, and after it the keyboard is the canvas's (a plain hotkey, F2, zooms it). Last, a
dialog KiCad brings up while the person types in Backplane's palette (an agent's RunAction: Plot) must leave the keyboard where it is.
Prints `CHECK 0|1 what (detail)` lines."""
import json, os, subprocess, sys, time
OUT = sys.argv[1]
LIB = os.environ["KEMBED_LIB"]; DRV = os.environ["DRIVER_PY"]; IPC = os.environ["IPC_PY"]
# Board Setup's button in the top toolbar, from the frame's top left (the 1800x1050 layout, Nov 15); a button in a dialog is found by its picture
TB_BUTTON = (14 + 28 * 2, 48)

def sh(*a): return subprocess.run(list(a), capture_output=True, text=True).stdout
def rt(*a): return json.loads(sh(sys.executable, os.path.join(LIB, "rt_input.py"), *map(str, a)) or "{}")
def ipc(*a): return sh(sys.executable, IPC, *map(str, a))
def check(ok, what, detail=""): print(f"CHECK {int(bool(ok))} {what}" + (f" ({detail})" if detail else ""), flush=True)
def log(): return open(os.path.join(OUT, "client.log"), errors="ignore").read()
def report(tag):
    ls = [l for l in log().splitlines() if f'"tag":"{tag}"' in l and '"event":"kicad"' in l]
    return json.loads(ls[-1][ls[-1].index("{"):]) if ls else {}
import ctypes, ctypes.util
X = ctypes.CDLL(ctypes.util.find_library("X11")); X.XOpenDisplay.restype = ctypes.c_void_p
D = ctypes.c_void_p(X.XOpenDisplay(os.environ["DISPLAY"].encode()))
def xfocus():
    w = ctypes.c_ulong(); r = ctypes.c_int(); X.XGetInputFocus(D, ctypes.byref(w), ctypes.byref(r)); return w.value
def inside(f, w):
    w = int(w, 16) if isinstance(w, str) else int(w); root = ctypes.c_ulong(); par = ctypes.c_ulong(); ch = ctypes.POINTER(ctypes.c_ulong)(); n = ctypes.c_uint(); c = f
    for _ in range(30):
        if c == w: return True
        if not X.XQueryTree(D, ctypes.c_ulong(c), ctypes.byref(root), ctypes.byref(par), ctypes.byref(ch), ctypes.byref(n)) or par.value in (0, root.value): return False
        c = par.value
    return False
def win_image(name, w):
    """Window w's own picture (works where the root window has none: mutter's rootless Xwayland)."""
    p = os.path.join(OUT, f"{name}.png"); subprocess.run(["import", "-window", str(w), p], capture_output=True); return p
def crop(name, x, y, w, h):
    p = os.path.join(OUT, f"{name}.png"); subprocess.run(["import", "-window", "root", "-crop", f"{w}x{h}+{x}+{y}", p], capture_output=True); return p
def changed(a, b):
    from PIL import Image, ImageChops
    return ImageChops.difference(Image.open(a).convert("RGB"), Image.open(b).convert("RGB")).convert("L").point(lambda v: 255 if v > 24 else 0).histogram()[255]

r = report("r"); pid = r.get("pid"); frame = r.get("shown"); sock = r.get("sock")
pane = r.get("pane") or {}; sr = r.get("shown_root") or {}
X0, Y0, W, H = sr.get("x", 0), sr.get("y", 0), pane.get("w", 800), pane.get("h", 900)
EX, EY = X0 + 70, Y0 + H - 90  # an empty spot of the canvas (real_use's)
print(f"INFO KiCad {pid}, PCB frame {frame} at {X0},{Y0} {W}x{H}", flush=True)
def dialogs():
    ws = json.loads(sh(sys.executable, DRV, "list-windows", "--pid", str(pid)) or "[]")
    return [w for w in ws if w.get("shown") and not w.get("override_redirect") and w["xid"] not in (frame, hex(frame)) and w["w"] >= 200 and w["h"] >= 100 and "PCB Editor" not in (w.get("title") or "")]
def xid(w): return int(w["xid"], 16) if isinstance(w["xid"], str) else int(w["xid"])
def wait_new(before, s=4.0):
    t = time.time()
    while time.time() - t < s:
        new = [w for w in dialogs() if xid(w) not in before]
        if new: return new[-1]
        time.sleep(0.1)
def wait_gone(w, s=3.0):
    t = time.time()
    while time.time() - t < s:
        if not any(xid(d) == xid(w) for d in dialogs()): return True
        time.sleep(0.1)
    return False
def has_keys(w, s=1.5):  # the keyboard in w within s
    t = time.time()
    while time.time() - t < s:
        if inside(xfocus(), xid(w)): return True
        time.sleep(0.1)
    return False
def canvas_keys(tag):  # a plain hotkey (F2, zoom out) over the canvas acts: the keyboard is the canvas's
    hx, hy = X0 + W * 3 // 4, Y0 + H // 2
    subprocess.run([sys.executable, os.path.join(os.environ["REPO_ROOT"], "test/cef/xinput.py"), "move", str(hx), str(hy)], capture_output=True); time.sleep(0.4)
    # KiCad's frame's own picture (the screen's has none under mutter's rootless Xwayland)
    a = win_image(f"{tag}_a", frame); rt("key", "F2"); time.sleep(1.0); b = win_image(f"{tag}_b", frame); rt("key", "Home"); time.sleep(0.8)
    if not (os.path.exists(a) and os.path.exists(b)): return None  # no picture of the screen here (mutter's rootless Xwayland has no root image)
    return changed(a, b)
def find_button(d, fixture):
    """A button in dialog `d` found by its picture (lib/fixtures/<fixture>.png), scored on its lettering only (its bright pixels: the buttons
    share their background, so a whole-picture score took a neighbour): a dialog's size and place vary by a few pixels between runs."""
    from PIL import Image, ImageChops, ImageStat
    btn = Image.open(os.path.join(LIB, "fixtures", f"{fixture}.png")).convert("L")
    x0, y0 = root_xy(xid(d))
    region = (x0, y0 + d["h"] // 2, d["w"], d["h"] - d["h"] // 2)  # its lower half
    whole = win_image("subsearch", xid(d))
    if not os.path.exists(whole): return 0, 0, 1e9
    shot = Image.open(whole).convert("L").crop((0, d["h"] // 2, d["w"], d["h"]))
    text = btn.point(lambda v: 255 if v > 120 else 0)
    best = (1e9, 0, 0)
    for yy in range(0, shot.height - btn.height):
        for xx in range(0, shot.width - btn.width, 2):
            m = ImageStat.Stat(ImageChops.difference(shot.crop((xx, yy, xx + btn.width, yy + btn.height)), btn), text).mean[0]
            if m < best[0]: best = (m, xx, yy)
    return region[0] + best[1] + btn.width // 2, region[1] + best[2] + btn.height // 2, best[0]
def root_xy(w):
    """A window's top left on the screen, from X (the list's root_x is not always there: a dialog in a window manager's frame)."""
    x, y, child = ctypes.c_int(), ctypes.c_int(), ctypes.c_ulong()
    X.XDefaultRootWindow.restype = ctypes.c_ulong; X.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    X.XTranslateCoordinates(D, ctypes.c_ulong(w), ctypes.c_ulong(X.XDefaultRootWindow(D)), 0, 0, ctypes.byref(x), ctypes.byref(y), ctypes.byref(child))
    return x.value, y.value
def close_any():
    for d in dialogs():
        x0, y0 = root_xy(xid(d)); rt("click", x0 + d["w"] // 2, y0 + 20); time.sleep(0.3); rt("key", "Escape"); time.sleep(0.6)
def before_dialog():
    """What left a dialog without the keyboard: the palette, the comment window, the palette, then back to the canvas."""
    rt("click", EX, EY); time.sleep(0.6)
    rt("key", "k", "ctrl"); time.sleep(1.2); rt("key", "Escape"); time.sleep(0.5); rt("click", EX, EY); time.sleep(0.6)
    rt("key", "semicolon", "ctrl"); time.sleep(1.5); rt("key", "Escape"); time.sleep(0.6); rt("click", EX, EY); time.sleep(0.6)
    rt("key", "k", "ctrl"); time.sleep(1.2); rt("key", "Escape"); time.sleep(0.5); rt("click", EX, EY); time.sleep(0.6)
ref = sh("sh", "-c", f"grep -o '(property \"Reference\" \"[URCJ][0-9]*\"' '{os.environ['PCB']}' | head -1").split('"')[3]
def open_by(kind):
    before = {xid(w) for w in dialogs()}
    if kind == "key":
        ipc("select-items", sock, "pcb", ref, "1"); time.sleep(0.8); rt("key", "e")
    else:
        rt("click", X0 + TB_BUTTON[0], Y0 + TB_BUTTON[1])
    return wait_new(before)

rounds = ["key", "toolbar", "key", "sub", "key", "key"]  # sub: Board Setup from the toolbar, then Import Settings from Another Board... in it
for i, kind in enumerate(rounds):
    before_dialog()
    how = {"key": "a key", "toolbar": "a toolbar click", "sub": "a toolbar click, then a dialog of it"}[kind]
    d = open_by(kind); name = f"round {i + 1} ({how})"
    check(d, f"{name}: a dialog came up", d.get("title") if d else "none")
    if not d: close_any(); continue
    check(has_keys(d), f"{name}: {d.get('title')!r} has the keyboard", f"X focus {xfocus()}")
    if kind == "sub":
        before = {xid(w) for w in dialogs()}
        # Board Setup's Import Settings from Another Board... opens a modal dialog over it (Footprint Properties' Change Footprint... does
        # not: KiCad closes the properties first)
        bx, by, score = find_button(d, "import_settings_button")
        print(f"INFO Import Settings from Another Board... found at {bx},{by} (mean difference {score:.1f})", flush=True)
        s = None
        if score > 40: check(False, f"{name}: Import Settings from Another Board... found in {d.get('title')!r}", f"best mean difference {score:.1f} at {bx},{by}")
        else: rt("click", bx, by); s = wait_new(before, 3)
        check(s, f"{name}: a click in it brings up another", s.get("title") if s else "none")
        if s:
            check(has_keys(s), f"{name}: {s.get('title')!r} has the keyboard", f"X focus {xfocus()}")
            rt("key", "Escape"); check(wait_gone(s), f"{name}: Escape closes {s.get('title')!r}")
            check(has_keys(d), f"{name}: the keyboard is back in {d.get('title')!r}", f"X focus {xfocus()}")
    rt("key", "Escape"); gone = wait_gone(d)
    check(gone, f"{name}: Escape closes {d.get('title')!r}")
    if not gone: close_any()
    time.sleep(0.5)
    check(inside(xfocus(), frame), f"{name}: the keyboard is back on the board editor", f"X focus {xfocus()}")
    n = canvas_keys(f"r{i + 1}")
    if n is None: print(f"INFO {name}: no screenshot on this display (rootless Xwayland): the hotkey's effect not read; the keyboard's place is checked above", flush=True)
    else: check(n > 500, f"{name}: a plain hotkey (F2) then acts on the canvas", f"{n} pixels changed")

# the person types in Backplane's palette; KiCad brings up a dialog meanwhile (an agent's RunAction): the keyboard stays with the person
rt("click", EX, EY); time.sleep(0.6); rt("key", "k", "ctrl"); time.sleep(1.2)
f0 = xfocus(); before = {xid(w) for w in dialogs()}
ipc("run-action", sock, "common.Control.plot"); d = wait_new(before, 6)
for k in "abc": rt("key", k); time.sleep(0.1)
time.sleep(1.0); f1 = xfocus()
check(d, "typing in the palette: KiCad's Plot dialog came up (an agent's action)", d.get("title") if d else "none")
check(d and not inside(f1, xid(d)) and f1 == f0, "typing in the palette: the dialog did not take the keyboard", f"X focus {f0} -> {f1}")
rt("key", "Escape"); time.sleep(0.5)
if d: close_any()
