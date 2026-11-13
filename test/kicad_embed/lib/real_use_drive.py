#!/usr/bin/env python3
"""The real-use journeys, driven from outside the client as a person would: XTest clicks on the client's controls (rects from its
`controls` metric), XTest pan/zoom/keys into KiCad, an agent's commits over KiCad's API, the hub's RPCs; the pane's pixels sampled
throughout (pane_sampler.py). Writes $OUT/real_use.json and prints PERF lines.
  real_use_drive.py OUT CLIENT_LOG JOURNEYS     JOURNEYS: comma list of open,switch,edit,coedit,xprobe,comment,save,sch (default all)
Environment from env_xvfb.sh: DISPLAY, HPORT, IPC_PY, DRIVER_PY, HUB_RPC_MJS.
A KiCad dialog that appears on its own (a load warning) is recorded with its title and time and dismissed with Return: it blocks KiCad."""
import json, os, re, subprocess, sys, threading, time

OUT, CLOG = sys.argv[1], sys.argv[2]
JOURNEYS = (sys.argv[3] if len(sys.argv) > 3 else "focus,open,switch,edit,mgmt,coedit,xprobe,comment,save,tools,textkeys,editors,plan,undo,checks,markers,panels,sch").split(",")
LIB = os.path.dirname(os.path.abspath(__file__))
IPC, DRV, RPC = os.environ["IPC_PY"], os.environ["DRIVER_PY"], os.environ["HUB_RPC_MJS"]
RT, SAMP = os.path.join(LIB, "rt_input.py"), os.path.join(LIB, "pane_sampler.py")
HPORT = os.environ["HPORT"]
now = lambda: round(time.time() * 1000, 1)
EVENTS, RESULTS, DIALOGS = [], {}, []

def sh(*a, timeout=60):
    try: return subprocess.run([str(x) for x in a], capture_output=True, text=True, timeout=timeout).stdout
    except subprocess.TimeoutExpired: return ""
def jl(text):
    for l in reversed((text or "").strip().splitlines()):
        try: return json.loads(l)
        except Exception: pass
    return {}
def rt(*a): return jl(sh("python3", RT, *a))
def ipc(*a, timeout=30): return jl(sh("python3", IPC, *a, timeout=timeout))
import shutil
BUN = shutil.which("bun") or os.path.expanduser(os.environ.get("REAL_HOME", "~") + "/.bun/bin/bun")
def rpc(steps): return sh(BUN, RPC, HPORT, os.path.join(OUT, "home", "token"), json.dumps(steps), timeout=30)
def perf(name, val, **kw):
    RESULTS[name] = {"ms": val, **kw}
    extra = " ".join(f"{k}={v}" for k, v in kw.items())
    print(f"PERF {name} {val}ms {extra}".rstrip(), flush=True)

def metrics(event):
    out = []
    for l in open(CLOG, errors="ignore"):
        if "GPUI_METRIC " in l and f'"event":"{event}"' in l:
            try: out.append(json.loads(l.split("GPUI_METRIC ", 1)[1]))
            except Exception: pass
    return out
def control(cid):
    for m in reversed(metrics("controls")):
        for c in m.get("controls", []):
            if c.get("id") == cid and c.get("rect"): return c["rect"]
    return None
def controls_like(pat):
    for m in reversed(metrics("controls")):
        return [c for c in m.get("controls", []) if re.search(pat, c.get("id", "")) and c.get("rect")]
    return []
GX = GY = 0
PANE = {}
WARNED = set()
# the editor's tabs register no control rects (kicad_pane.rs): their place over the pane, measured from a screenshot at 1800x1050
TAB_AT = {"PCB": 31, "SCH": 92, "PCB_3D": 146}
def click_ctl(cid):
    r = control(cid)
    if not r and cid in ("kicad:full", "kicad:popout") and PANE:
        pn = pane_now(); off = {"kicad:full": 114, "kicad:popout": 39}[cid]  # measured at 1800x1050 from the pane's right edge
        return rt("click", pn["x"] + pn["w"] - off, pn["y"] - 15).get("t0")
    if not r and cid.startswith("kicad-tab:") and PANE:
        print(f"DIAG {cid}: no rect, clicking its measured place", flush=True) if cid not in WARNED else None; WARNED.add(cid)
        return rt("click", PANE["x"] + TAB_AT[cid.split(":", 1)[1]], PANE["y"] - 15).get("t0")
    if not r: print(f"DIAG no control {cid}", flush=True); return None
    return rt("click", GX + r["x"] + r["w"] / 2, GY + r["y"] + r["h"] / 2).get("t0")
def pane_now():
    """The pane as it is now: the widest KiCad frame mapped near the pane's top (Full size widens it); else PANE."""
    best = None
    for w in windows():
        if w.get("shown") and "kicad" in w.get("wm_class", "").lower() and w["w"] > 300 and w["h"] > 300 and abs(w["root_y"] - PANE.get("y", 0)) < 60 and w["root_x"] >= 0:
            if not best or w["w"] > best["w"]: best = w
    return {"x": best["root_x"], "y": best["root_y"], "w": best["w"], "h": best["h"]} if best else PANE
def kicad_report(tag):
    rs = [m for m in metrics("kicad") if m.get("tag") == tag]
    return rs[-1] if rs else {}
def windows(): return json.loads(sh("python3", DRV, "list-windows") or "[]")

def xfocus():
    """X's keyboard focus: the window and its parents up to the root, each as (id, WM_NAME, WM_CLASS)."""
    import ctypes, ctypes.util
    x = ctypes.CDLL(ctypes.util.find_library("X11")); vp, ul = ctypes.c_void_p, ctypes.c_ulong
    x.XOpenDisplay.restype = vp; x.XOpenDisplay.argtypes = [ctypes.c_char_p]
    d = x.XOpenDisplay(os.environ.get("DISPLAY", ":0").encode())
    if not d: return []
    w = ul(); rev = ctypes.c_int(); x.XGetInputFocus.argtypes = [vp, ctypes.POINTER(ul), ctypes.POINTER(ctypes.c_int)]
    x.XGetInputFocus(d, ctypes.byref(w), ctypes.byref(rev))
    x.XFetchName.argtypes = [vp, ul, ctypes.POINTER(ctypes.c_char_p)]
    x.XQueryTree.argtypes = [vp, ul, ctypes.POINTER(ul), ctypes.POINTER(ul), ctypes.POINTER(vp), ctypes.POINTER(ctypes.c_uint)]
    class CH(ctypes.Structure): _fields_ = [("name", ctypes.c_char_p), ("cls", ctypes.c_char_p)]
    x.XGetClassHint.argtypes = [vp, ul, ctypes.POINTER(CH)]
    out, cur = [], w.value
    for _ in range(12):
        if cur in (0, 1): out.append(hex(cur)); break
        nm = ctypes.c_char_p(); x.XFetchName(d, cur, ctypes.byref(nm)); ch = CH(); x.XGetClassHint(d, cur, ctypes.byref(ch))
        out.append([hex(cur), (nm.value or b"").decode("utf-8", "replace")[:40], (ch.cls or b"").decode()])
        root, par, kids, n = ul(), ul(), vp(), ctypes.c_uint()
        if not x.XQueryTree(d, cur, ctypes.byref(root), ctypes.byref(par), ctypes.byref(kids), ctypes.byref(n)) or par.value in (0, root.value): break
        cur = par.value
    x.XCloseDisplay.argtypes = [vp]; x.XCloseDisplay(d)
    return out
def shot(name): sh("import", "-window", "root", os.path.join(OUT, "shots", f"{name}.png"), timeout=20)
# --- dialogs: KiCad windows that are neither an editor frame nor embedded: recorded, then Return
STOP = threading.Event(); KNOWN = set(); PAUSE = threading.Event()  # PAUSE: a journey opens KiCad's dialogs on purpose
def dialog_watch():
    while not STOP.is_set():
        if PAUSE.is_set(): STOP.wait(0.25); continue
        try:
            for w in windows():
                if not w.get("shown") or w.get("override_redirect") or w["w"] < 120 or w["h"] < 60: continue
                if "kicad" not in w.get("wm_class", "").lower(): continue
                t = w.get("title", "")
                if re.search(r"(PCB Editor|Schematic Editor|3D Viewer|Footprint Editor|Symbol Editor)$", t) or w["xid"] in KNOWN: continue
                if not w.get("transient_for") and t not in ("Load Schematic", "Load PCB", "Error", "Warning", "Confirmation", "Information", "File Open Warning"): continue
                KNOWN.add(w["xid"]); DIALOGS.append({"t": now(), "title": t, "w": w["w"], "h": w["h"]})
                print(f"DIALOG {t!r} {w['w']}x{w['h']} (Return)", flush=True); shot(f"dialog{len(DIALOGS)}")
                sh("python3", os.path.join(os.environ["REPO_ROOT"], "test/cef/xinput.py"), "focus", t); rt("key", "Return")
        except Exception as e: print("DIAG dialog watch", e, flush=True)
        STOP.wait(0.25)

# --- sampler over the pane
SAMPLER = None
def sampler_start(rect, name, min_ms=0):
    global SAMPLER
    stop = os.path.join(OUT, f"{name}.stop")
    if os.path.exists(stop): os.remove(stop)
    path = os.path.join(OUT, f"{name}.jsonl")
    p = subprocess.Popen(["python3", SAMP, "run", str(rect["x"]), str(rect["y"]), str(rect["w"]), str(rect["h"]), path, stop, "1800"], env={**os.environ, "KEMBED_SAMPLE_MIN_MS": str(min_ms)})
    SAMPLER = (p, stop, path); time.sleep(0.3)
def sampler_stop():
    p, stop, path = SAMPLER; open(stop, "w").close(); p.wait(timeout=10); return path
def analyze(evs, path):
    ef = path + ".events.json"; json.dump(evs, open(ef, "w"))
    return json.loads(sh("python3", SAMP, "analyze", path, ef, timeout=300) or "[]")

def logs(key, val=None):
    """GPUI_LOG objects with this key (and value): the comment window's open/painted/capture/post/closed."""
    out = []
    for l in open(CLOG, errors="ignore"):
        if "GPUI_LOG " in l and f'"{key}"' in l:
            try: o = json.loads(l.split("GPUI_LOG ", 1)[1])
            except Exception: continue
            if val is None or o.get(key) == val: out.append(o)
    return out
def wait_log(key, val, after_n, secs):
    t = time.time()
    while time.time() - t < secs:
        got = logs(key, val)
        if len(got) > after_n: return got[-1]
        time.sleep(0.02)
    return None

# --- memory: the embedded KiCad's process tree, RSS every second; peak = the most at once, steady = after the editors are up and idle
MEM = {"samples": [], "pid": 0}
def tree_rss(pid):
    kids = {}
    for d in os.listdir("/proc"):
        if not d.isdigit(): continue
        try: pp = int(open(f"/proc/{d}/stat").read().rsplit(")", 1)[1].split()[1]); kids.setdefault(pp, []).append(int(d))
        except Exception: pass
    tot, hwm, todo = 0, 0, [pid]
    while todo:
        q = todo.pop()
        try:
            st = open(f"/proc/{q}/status").read()
            tot += int(re.search(r"VmRSS:\s+(\d+)", st)[1]); hwm += int(re.search(r"VmHWM:\s+(\d+)", st)[1])
        except Exception: pass
        todo += kids.get(q, [])
    return tot // 1024, hwm // 1024
def mem_watch():
    while not STOP.is_set():
        if MEM["pid"]:
            rss, hwm = tree_rss(MEM["pid"])
            if rss: MEM["samples"].append((now(), rss, hwm))
        STOP.wait(1.0)
def mem_now(): return tree_rss(MEM["pid"])[0] if MEM["pid"] else None

# --- a window's sizes over a span (xwininfo every 25 ms): how many times KiCad's container was resized
def sizes_during(xid, secs):
    seen, t = [], time.time()
    while time.time() - t < secs:
        o = sh("xwininfo", "-id", str(xid), timeout=5)
        w = re.search(r"Width:\s+(\d+)", o); h = re.search(r"Height:\s+(\d+)", o)
        if w and h and (not seen or seen[-1] != (w[1], h[1])): seen.append((w[1], h[1]))
        time.sleep(0.025)
    return seen

# --- wait for the client to have the editor up (its script opened it: cold Edit, measured by the script's wait_painted)
def wait_metric(pred, secs):
    t = time.time()
    while time.time() - t < secs:
        for m in metrics("kicad") + metrics("mark"):
            if pred(m): return m
        time.sleep(0.2)
    return None

threading.Thread(target=dialog_watch, daemon=True).start()
# before Edit: the pane's place sampled (where KiCad will show: 1800x1050 layout), and whether any KiCad window is visible anywhere
PRE_RECT = {"x": 924, "y": 98, "w": 804, "h": 896}
sampler_start(PRE_RECT, "pre_edit")
BEFORE = []
def before_watch():
    while not metrics("click") or not any(m.get("id") == "ui:Edit" for m in metrics("click")):
        for w in windows():
            if w.get("shown") and "kicad" in w.get("wm_class", "").lower() and w["w"] > 50 and w["h"] > 50 and w["xid"] not in [b["xid"] for b in BEFORE]:
                BEFORE.append({"xid": w["xid"], "title": w.get("title"), "at": [w["root_x"], w["root_y"], w["w"], w["h"]], "t": now()})
                print(f"JANK a KiCad window visible before Edit: {w.get('title')!r} at {w['root_x']},{w['root_y']} {w['w']}x{w['h']}", flush=True)
        time.sleep(0.15)
bw = threading.Thread(target=before_watch, daemon=True); bw.start()
cold = wait_metric(lambda m: m.get("tag") == "cold", 180)
if not cold: print("FAIL no cold report"); sys.exit(1)
edit_click = next((m for m in metrics("click") if m.get("id") == "ui:Edit"), {})
perf("open_cold_painted", cold.get("wait_ms"), state=cold.get("state"))
time.sleep(1.5); pre_path = sampler_stop()
RESULTS["kicad_visible_before_edit"] = {"ms": None, "ok": int(not BEFORE), "windows": BEFORE}
if edit_click.get("epoch_ms"):
    r = analyze([{"kind": "switch", "t0": edit_click["epoch_ms"], "tag": "first_edit", "window_ms": 8000}], pre_path)
    if r: r = r[0]; perf("first_edit_pixels", r.get("settled_ms"), first_change_ms=r.get("first_change_ms"), blank_frames=r.get("blank_frames"), mid_frames=r.get("mid_frames"))
wait_metric(lambda m: m.get("name") == "ready_for_driver", 60)
geo = kicad_report("geom") or cold
for w in windows():
    if w.get("xid_int") == geo.get("toplevel"): GX, GY = w["root_x"], w["root_y"]
pane = geo.get("pane") or {}
PANE = {"x": GX + int(pane.get("x", 0)), "y": GY + int(pane.get("y", 0)), "w": int(pane.get("w", 0)), "h": int(pane.get("h", 0))}
SOCK = geo.get("sock")
PCB = next((f.get("doc") for f in geo.get("frames", []) if f.get("kind") == "PCB" and f.get("doc")), "") or ""
import glob
PROJ_DIR = os.path.join(OUT, "proj")
if not PCB: PCB = (sorted(glob.glob(os.path.join(PROJ_DIR, "*.kicad_pcb"))) or [""])[0]
KPRO = PCB[:-len(".kicad_pcb")] + ".kicad_pro" if PCB else ""
MEM["pid"] = int(geo.get("pid") or 0); threading.Thread(target=mem_watch, daemon=True).start()
# the KiCad actually running (never the one asked for): its executable, its runtime's SOURCE_VERSION/SOURCE_COMMIT, and its API's version
KICAD_RUN = {"api": ipc("version", SOCK).get("full_version") if SOCK else None}
try:
    exe = os.readlink(f"/proc/{MEM['pid']}/exe"); rt_dir = os.path.dirname(os.path.dirname(exe)); KICAD_RUN["exe"] = exe
    for f in ("SOURCE_VERSION", "SOURCE_COMMIT", "BACKPLANE_COMMIT"):
        if os.path.exists(os.path.join(rt_dir, f)): KICAD_RUN[f.lower()] = open(os.path.join(rt_dir, f)).read().strip()
except Exception as e: KICAD_RUN["error"] = str(e)
asked = os.environ.get("KICAD_BIN", "")
KICAD_RUN["asked"] = asked
if asked and KICAD_RUN.get("exe") and os.path.dirname(os.path.dirname(os.path.realpath(asked))) != os.path.dirname(os.path.dirname(KICAD_RUN["exe"])):
    print(f"FAIL the KiCad running is not the one asked for: {KICAD_RUN}", flush=True); RESULTS["kicad_mismatch"] = {"ms": None, "ok": 0}
# the runtime asked for is known by its BACKPLANE_COMMIT (bp.13 builds all read "10.99.0-unknown-bp.13"): the running one must carry the same
want_bc = ""
try: want_bc = open(os.path.join(os.path.dirname(os.path.dirname(os.path.realpath(asked))), "BACKPLANE_COMMIT")).read().strip()
except Exception: pass
if want_bc and KICAD_RUN.get("backplane_commit") != want_bc:
    print(f"FAIL the KiCad running is BACKPLANE_COMMIT {KICAD_RUN.get('backplane_commit')}, asked for {want_bc}", flush=True); RESULTS["kicad_mismatch"] = {"ms": None, "ok": 0}
print(f"INFO kicad {KICAD_RUN}", flush=True)
# after Edit and the other editors' pre-warm, before any edit: is any document marked modified (a pre-warm that "repairs" a file)?
if SOCK:
    pm = {k: ipc("modified", SOCK, k).get("modified") for k in ("pcb", "sch")}
    RESULTS["prewarm_modified"] = {"ms": None, "ok": int(not any(v is True for v in pm.values())), "pcb": pm["pcb"], "sch": pm["sch"]}
    print(f"INFO modified after pre-warm: {pm}", flush=True)
time.sleep(1.2); RESULTS["rss_ready_mb"] = {"ms": None, "mb": mem_now()}
CONT = int((geo.get("container") or {}).get("xid") or 0)
print(f"INFO pane={PANE} sock={SOCK} kicad_pid={MEM['pid']} rss={RESULTS['rss_ready_mb']['mb']}MB gpu={(metrics('gpu') or [{}])[0].get('device')} client={os.environ.get('CLIENT_BIN')}", flush=True)
CX, CY = PANE["x"] + PANE["w"] // 2, PANE["y"] + PANE["h"] // 2
sampler_start(PANE, "pane")
_sn = (metrics("shown") or [{}])[-1]
if _sn.get("thread") != "t1" or _sn.get("open") != "board":
    print(f"FAIL the editor is not open on t1 (Embedded KiCad E2E): shown={_sn}", flush=True); sys.exit(1)

def editor_up():  # a KiCad editor frame mapped over the pane (the X tree, not the client's word)
    for w in windows():
        if w.get("shown") and "kicad" in w.get("wm_class", "").lower() and w["w"] > 200 and abs(w["root_x"] - PANE["x"]) < 40 and abs(w["root_y"] - PANE["y"]) < 60: return True
    return False
def container_now():  # the X parent of the KiCad frame over the pane: Backplane's container (a reopen makes a new one)
    for w in windows():
        if w.get("shown") and "kicad" in w.get("wm_class", "").lower() and w["w"] > 200 and abs(w["root_x"] - PANE["x"]) < 40 and abs(w["root_y"] - PANE["y"]) < 60:
            return int(w["parent"], 16) if isinstance(w["parent"], str) else w["parent"]
    return CONT
def restore(after):
    if editor_up(): return
    print(f"LOST editor after {after}: reopening (Board, Edit)", flush=True); RESULTS[f"lost_after_{after}"] = {"ms": None, "lost": 1}
    click_ctl("rail:Board"); time.sleep(2); click_ctl("ui:Edit"); time.sleep(4)
TITLE_KIND = [("PCB Editor", "PCB"), ("Schematic Editor", "SCH"), ("3D Viewer", "PCB_3D"), ("Symbol Editor", "SYMBOL_EDITOR"), ("Footprint Editor", "FOOTPRINT_EDITOR"), ("Gerber Viewer", "GERBVIEW")]
def on_show():
    """The kinds of the KiCad frames shown at the pane's place now (from the X tree, not the client's word)."""
    out = []
    for w in windows():
        if not w.get("shown") or "kicad" not in w.get("wm_class", "").lower() or not PANE: continue
        if abs(w["root_x"] - PANE["x"]) > 2 or abs(w["root_y"] - PANE["y"]) > 2: continue
        t = (w.get("title") or "").rstrip()
        out += [k for (s, k) in TITLE_KIND if t.endswith(s) or (s == "3D Viewer" and t.startswith("3D Viewer"))]
    return out
SHOW_MISSES = []
def tab(kind, tag, settle=1.4, check=True):
    t0 = click_ctl(f"kicad-tab:{kind}")
    if t0: EVENTS.append({"kind": "switch", "t0": t0, "tag": tag, "window_ms": settle * 1000}); time.sleep(settle)
    # the switch's end state: the asked-for editor is what the pane shows (a frame made but never shown failed only a paint wait before)
    if t0 and check:
        got = on_show()
        if got != [kind]:
            time.sleep(3); got = on_show()
            if got != [kind]:
                SHOW_MISSES.append({"tag": tag, "want": kind, "shown": got}); print(f"JANK {tag}: asked for {kind}, the pane shows {got or 'no KiCad frame'} {settle + 3:.1f} s after the press", flush=True)
            else: print(f"JANK {tag}: {kind} on show only after {settle + 3:.1f} s", flush=True)

XIN = os.path.join(os.environ["REPO_ROOT"], "test/cef/xinput.py")
def comment_window(tag):
    """Ctrl+; with the keyboard in KiCad: the key to the comment window's first paint (GPUI_LOG kcomment painted at_ms); then Escape."""
    n = len(logs("kcomment", "painted")); r = rt("key", "semicolon", "ctrl")
    got = wait_log("kcomment", "painted", n, 2.0)
    ms = round(got["at_ms"] - r["t0"], 1) if got and got.get("at_ms") else None
    perf(tag, ms if ms is not None else -1, opened=int(got is not None))
    if got: time.sleep(0.4); rt("key", "Escape"); time.sleep(0.6)
    return got is not None
# 0. focus: the first click into KiCad, then a reserved chord (Ctrl+;) must reach Backplane at once (the first-click bug: it took a second click)
# an empty spot of the board canvas (bottom left, inside the left toolbar and above the message panel; the board is zoomed to fit, so
# its margin): a click there selects nothing, so KiCad has no Clarify Selection to ask and a menu after it is a bug
EX, EY = PANE["x"] + 70, PANE["y"] + PANE["h"] - 90
def kicad_focused():
    """Which KiCad frame has the keyboard now, KiCad's own word (FrameInfo.focused), and the X focus."""
    fr = ipc("frames", SOCK).get("frames", []) if SOCK else []
    return [f.get("kind") for f in fr if f.get("focused")]
if "focus" in JOURNEYS:
    # Edit gives KiCad the keyboard: with no click into the canvas, KiCad's frame is focused and a reserved chord pressed now reaches
    # Backplane through KiCad (lane A: after Edit the keyboard stayed in Backplane's window, so KiCad's hotkeys reached nothing)
    fk = kicad_focused(); print(f"INFO after Edit, no click: KiCad focused frames {fk}", flush=True)
    RESULTS["edit_gives_kicad_keys"] = {"ms": None, "ok": int(bool(fk)), "focused": fk}
    if not fk: print("JANK after Edit: no KiCad frame has the keyboard", flush=True)
    t0 = click_ctl("kicad:full"); time.sleep(1.2)
    fk2 = kicad_focused(); RESULTS["full_size_gives_kicad_keys"] = {"ms": None, "ok": int(bool(fk2)), "focused": fk2}
    if not fk2: print("JANK after Full size: no KiCad frame has the keyboard", flush=True)
    click_ctl("kicad:full"); time.sleep(1.2)
    rt("click", EX, EY); time.sleep(0.6)
    # Ctrl+K (Backplane's command palette, a reserved key) right after the one click: the palette's place over the chat changes?
    box = {"x": 600, "y": 0, "w": 700, "h": 420}
    sampler_stop(); sampler_start(box, "ctrlk"); r = rt("key", "k", "ctrl"); print(f"INFO Ctrl+K sent at {r.get('t0')}", flush=True); time.sleep(1.2); kp = sampler_stop(); sampler_start(PANE, "pane")
    a = analyze([{"kind": "commit", "t0": r["t0"], "tag": "ctrlk", "window_ms": 1000}], kp)
    ck = a[0].get("first_change_ms") if a else None
    perf("first_click_ctrl_k", ck if ck is not None else -1, opened=int(ck is not None)); RESULTS["first_click_ctrl_k_ok"] = {"ms": None, "ok": int(ck is not None)}
    rt("key", "Escape"); time.sleep(0.4); rt("click", EX, EY); time.sleep(0.6)
    ok1 = comment_window("first_click_ctrl_semicolon")
    RESULTS["first_click_reserved"] = {"ms": None, "ok": int(ok1)}
    if not ok1:
        print("JANK first click into KiCad: Ctrl+; did not reach Backplane; a second click:", flush=True)
        rt("click", EX, EY); time.sleep(0.6)
        RESULTS["second_click_reserved"] = {"ms": None, "ok": int(comment_window("second_click_ctrl_semicolon"))}
    # after a comment window: a reserved key KiCad hands over still reaches Backplane's actions (lane D: the panels' keys replayed as a
    # keystroke reached nothing after Ctrl+;); Ctrl+K opens the palette
    rt("click", EX, EY); time.sleep(0.6)
    box = {"x": 600, "y": 0, "w": 700, "h": 420}
    sampler_stop(); sampler_start(box, "ctrlk_after_comment"); r = rt("key", "k", "ctrl"); time.sleep(1.2); kp = sampler_stop(); sampler_start(PANE, "pane")
    a = analyze([{"kind": "commit", "t0": r["t0"], "tag": "ctrlk_after_comment", "window_ms": 1000}], kp)
    ck2 = a[0].get("first_change_ms") if a else None
    perf("ctrl_k_after_comment", ck2 if ck2 is not None else -1, opened=int(ck2 is not None)); RESULTS["ctrl_k_after_comment_ok"] = {"ms": None, "ok": int(ck2 is not None)}
    if ck2 is not None: rt("key", "Escape"); time.sleep(0.4)
# 0a. a KiCad dialog keeps the keyboard: a footprint selected, `e` opens its properties dialog (KiCad's own top-level), Escape closes it
# (lane D: the keyboard was taken back from Symbol Properties, so Escape reached Backplane's window and the dialog stayed)
if "focus" in JOURNEYS and SOCK:
    refs = [r for r in re.findall(r'\(property "Reference" "([A-Z]+[0-9]+)"', open(PCB, errors="ignore").read()) if r[0] in "URCJ"] if PCB else []
    if refs:
        rt("click", EX, EY); time.sleep(0.5)
        ipc("select-items", SOCK, "pcb", refs[0], "1"); time.sleep(0.8)
        rt("key", "e"); dlg = None
        for _ in range(40):
            dlg = next((w for w in windows() if w.get("shown") and "Properties" in (w.get("title") or "") and "kicad" in w.get("wm_class", "").lower()), None)
            if dlg: break
            time.sleep(0.1)
        if not dlg:
            print(f"DIAG dialog escape: no properties dialog for {refs[0]} after `e`", flush=True); RESULTS["dialog_escape"] = {"ms": None, "ok": 0, "opened": 0}
        else:
            time.sleep(0.8); t0 = rt("key", "Escape").get("t0"); gone_ms = None
            for _ in range(30):
                if not any(w.get("xid") == dlg["xid"] and w.get("shown") for w in windows()): gone_ms = round(now() - t0); break
                time.sleep(0.1)
            RESULTS["dialog_escape"] = {"ms": gone_ms, "ok": int(gone_ms is not None), "opened": 1, "title": dlg.get("title")}
            print(f"PERF dialog_escape {gone_ms}ms title={dlg.get('title')!r}", flush=True)
            if gone_ms is None:
                print(f"JANK {dlg.get('title')!r}: Escape did not close it (3 s): clicking its Cancel", flush=True); rt("key", "Escape")
# 0b. popup keys (bp.13 batch 4, embed-popup-keys): a KiCad popup (a right-click menu) holds the keyboard; Ctrl+K and Ctrl+; pressed then close
# it and reach Backplane. The menu is confirmed up first (another client holds the keyboard: an XGrabKeyboard from here is refused)
def kb_held():
    import ctypes, ctypes.util
    x = ctypes.CDLL(ctypes.util.find_library("X11")); x.XOpenDisplay.restype = ctypes.c_void_p; x.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x.XDefaultRootWindow.argtypes = [ctypes.c_void_p]; x.XDefaultRootWindow.restype = ctypes.c_ulong
    x.XGrabKeyboard.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
    x.XUngrabKeyboard.argtypes = [ctypes.c_void_p, ctypes.c_ulong]; x.XCloseDisplay.argtypes = [ctypes.c_void_p]
    d = x.XOpenDisplay(os.environ.get("DISPLAY", ":0").encode())
    if not d: return None
    st = x.XGrabKeyboard(d, x.XDefaultRootWindow(d), 0, 1, 1, 0)
    if st == 0: x.XUngrabKeyboard(d, 0)
    x.XCloseDisplay(d); return st != 0
if "focus" in JOURNEYS and SOCK:
    pc = ipc("capabilities", SOCK)
    if "embed-popup-keys" not in pc.get("capabilities", []):
        print("DIAG popup keys: this runtime has no embed-popup-keys: skipped", flush=True)
    else:
        for i, (keyname, tag) in enumerate([("k", "popup_ctrl_k"), ("semicolon", "popup_ctrl_semicolon")]):
            rt("click", EX, EY); time.sleep(0.4); rt("click", EX, EY, 3); time.sleep(0.8)
            up = kb_held(); print(f"INFO {tag}: right-click menu up (keyboard held by KiCad): {up}", flush=True)
            if keyname == "k":
                box = {"x": 600, "y": 0, "w": 700, "h": 420}
                sampler_stop(); sampler_start(box, tag); r = rt("key", "k", "ctrl"); time.sleep(1.2); kp = sampler_stop(); sampler_start(PANE, "pane")
                a = analyze([{"kind": "commit", "t0": r["t0"], "tag": tag, "window_ms": 1000}], kp)
                ok = a[0].get("first_change_ms") if a else None; perf(tag, ok if ok is not None else -1, menu_up=int(bool(up)), opened=int(ok is not None))
                got = ok is not None; rt("key", "Escape"); time.sleep(0.4)
            else:
                got = comment_window(tag)
            closed = kb_held() is False
            RESULTS[f"{tag}_ok"] = {"ms": None, "ok": int(bool(up) and got and closed), "menu_up": int(bool(up)), "reached": int(got), "menu_closed": int(closed)}
            if not (up and got and closed): print(f"JANK {tag}: menu_up={up} reached_backplane={got} menu_closed={closed}", flush=True)
# 1. open: the pane closed (View) and reopened (Edit) with KiCad warm
if "open" in JOURNEYS:
    for i in range(3):
        click_ctl("ui:View"); time.sleep(1.5)
        t0 = click_ctl("ui:Edit")
        if t0: EVENTS.append({"kind": "switch", "t0": t0, "tag": f"reopen{i}", "window_ms": 3000}); time.sleep(3)
# 2. switch: Board <-> Schematic <-> 3D, then threads away and back
if "switch" in JOURNEYS:
    seq = ["SCH", "PCB", "PCB_3D", "PCB", "SCH", "PCB_3D", "SCH", "PCB"] * 2
    for i, k in enumerate(seq): tab(k, f"tab{i:02d}_{k}")
    # threads away and back, by the palette (type the title, Return) and by the sidebar (or the project page's row). Every step asserts where
    # it is from the client's `shown` metric (the thread on show and its viewer) and the X tree (a KiCad frame over the pane), never an index
    def shown_now():
        m = metrics("shown"); return m[-1] if m else {}
    def where(label):
        sn = shown_now(); up = editor_up()
        print(f"INFO {label}: thread={sn.get('thread')} viewer={sn.get('open')} kicad_over_pane={up}", flush=True)
        return sn.get("thread"), sn.get("open"), up
    def jump(title):
        click_ctl("ui:Jump or run a command"); time.sleep(0.4)
        sh("python3", XIN, "type", title); time.sleep(0.5)
        return rt("key", "Return").get("t0")
    def side(tid):  # the sidebar's row when it lists the thread, else the project's row and its page's thread row
        if control(f"thread:{tid}"): return click_ctl(f"thread:{tid}")
        click_ctl("project:p1"); time.sleep(1.0)
        return click_ctl(f"page-thread:{tid}")
    def away_back(tag, go_away, go_back, n=3):
        for i in range(n):
            th, op, up = where(f"{tag}{i} before")
            if th != "t1" or op != "board" or not up:
                print(f"FAIL {tag}{i}: not on t1 with the editor before switching (thread={th} viewer={op} kicad={up})", flush=True)
                RESULTS[f"{tag}{i}_precondition"] = {"ms": None, "ok": 0}; restore(f"{tag}_pre"); continue
            if not go_away(): print(f"DIAG {tag}: could not switch away", flush=True); break
            time.sleep(1.5); th2, op2, up2 = where(f"{tag}{i} away")
            RESULTS[f"{tag}{i}_away_ok"] = {"ms": None, "ok": int(th2 == "t2" and not up2)}
            t0 = go_back()
            if t0: EVENTS.append({"kind": "switch", "t0": t0, "tag": f"{tag}{i}", "window_ms": 3000}); time.sleep(3)
            th3, op3, up3 = where(f"{tag}{i} back"); shot(f"{tag}{i}")
            RESULTS[f"{tag}{i}_kept"] = {"ms": None, "ok": int(th3 == "t1" and op3 == "board" and up3)}
            if not up3: print(f"JANK {tag}{i}: back on t1 but the editor is not there (viewer={op3})", flush=True); restore(f"{tag}{i}")
    away_back("thread_back", lambda: jump("Other thread 2"), lambda: jump("Embedded KiCad E2E"))
    away_back("thread_side_back", lambda: side("t2"), lambda: side("t1"))
shot("after_switch")
# 3. edit: pan and zoom 10 s each, a click (select), a drag (move), Delete, Ctrl+Z, a short interactive track
if "edit" in JOURNEYS:
    tab("PCB", "edit_board", 1.5)
    rt("click", CX, CY + PANE["h"] // 3)  # focus into KiCad (an empty spot low in the pane)
    time.sleep(0.5)
    r = rt("pan", CX, CY, min(PANE["w"], PANE["h"]) // 5, 10, 2, 120); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "pan_10s"}); time.sleep(1)
    r = rt("wheel", CX, CY, 10, 60); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "zoom_10s"}); time.sleep(1)
    rt("key", "Home"); time.sleep(1)  # zoom to fit
    r = rt("click", CX, CY); EVENTS.append({"kind": "switch", "t0": r["t0"], "tag": "select_click", "window_ms": 1500}); time.sleep(1.5)
    r = rt("drag", CX, CY, CX + 60, CY + 40, 600); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "drag_move"}); time.sleep(1)
    r = rt("key", "Delete"); EVENTS.append({"kind": "switch", "t0": r["t0"], "tag": "delete", "window_ms": 1500}); time.sleep(1.5)
    r = rt("key", "z", "ctrl"); EVENTS.append({"kind": "switch", "t0": r["t0"], "tag": "undo", "window_ms": 1500}); time.sleep(1.5)
    r = rt("key", "z", "ctrl"); time.sleep(1.5)
    rt("key", "Escape"); rt("click", CX - 100, CY + 80); rt("key", "x")  # route from an empty spot
    r = rt("pan", CX - 30, CY + 80, 70, 1.5, 0, 60)  # button 0: no button, the pointer moves while routing
    if r: EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "route_track"})
    rt("click", CX + 40, CY + 80); rt("key", "Escape"); rt("key", "Escape"); time.sleep(1)
    rt("key", "z", "ctrl"); time.sleep(1)
# 3b. what sampling costs: the same pan with the sampler at most every 20 ms (its own file); compare hitch counts with pan_10s
if "edit" in JOURNEYS:
    main_path = sampler_stop(); main_events = list(EVENTS); EVENTS.clear()
    sampler_start(PANE, "pane_slow", 20)
    r = rt("pan", CX, CY, min(PANE["w"], PANE["h"]) // 5, 6, 2, 120); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "pan_6s_sample20"}); time.sleep(1)
    slow = analyze(list(EVENTS), sampler_stop()); EVENTS.clear()
    PRE = analyze(main_events, main_path)
    sampler_start(PANE, "pane2")
else:
    slow, PRE = [], []
# 3c. management panels beside the editor: each panel's tab clicked open and shut, and Alt+Shift+B / Alt+Shift+C with Backplane's keyboard:
# click to settled, blank frames over the pane, and how many times KiCad's container changed size (once is right)
if "mgmt" in JOURNEYS:
    tab("PCB", "mgmt_board", 1.2)
    ids = sorted({c["id"] for c in controls_like(r"^mgmt:[a-z]+$") if c["id"] not in ("mgmt:right", "mgmt:bottom", "mgmt:left", "mgmt:close", "mgmt:shut")})
    print(f"INFO management panels: {ids}", flush=True)
    def panel(press, tag):
        box = {}
        cont = container_now()
        th = threading.Thread(target=lambda: box.update(s=sizes_during(cont, 2.4))) if cont else None
        if th: th.start(); time.sleep(0.1)
        t0 = press()
        if t0: EVENTS.append({"kind": "switch", "t0": t0, "tag": tag, "window_ms": 2000})
        if th: th.join()
        RESULTS[f"{tag}_resizes"] = {"ms": None, "resizes": max(0, len(box.get("s", [])) - 1), "sizes": " ".join("x".join(x) for x in box.get("s", []))}
        time.sleep(0.6)
    for pid_ in ids:
        name = pid_.split(":")[1]
        panel(lambda: click_ctl(pid_), f"mgmt_{name}_open"); shot(f"mgmt_{name}")
        panel(lambda: click_ctl(pid_), f"mgmt_{name}_close")
    rt("click", 600, 500); time.sleep(0.4)  # Backplane's keyboard (the chat), then the keys
    for key, name in (("b", "bom"), ("c", "checks")):
        panel(lambda: rt("key", key, "alt", "shift").get("t0"), f"mgmt_key_{name}_open"); shot(f"mgmt_key_{name}")
        panel(lambda: rt("key", key, "alt", "shift").get("t0"), f"mgmt_key_{name}_close")
    restore("mgmt")
# 4. co-edit: an agent's commits while idle, then while the person pans
if "coedit" in JOURNEYS and SOCK:
    tab("PCB", "coedit_board", 1.5)
    for i in range(5):
        t0 = now(); ipc("agent-add-track", SOCK, 100 + i, 100, 110 + i, 100, 0.25); EVENTS.append({"kind": "commit", "t0": t0, "tag": f"commit_idle{i}", "window_ms": 1500}); time.sleep(1.5)
    commits = []
    def pan_bg(): commits.append(rt("pan", CX, CY, min(PANE["w"], PANE["h"]) // 5, 8, 2, 120))
    th = threading.Thread(target=pan_bg); th.start(); time.sleep(1)
    for i in range(6): ipc("agent-add-track", SOCK, 100 + i, 110, 110 + i, 110, 0.25); time.sleep(1)
    th.join()
    if commits and commits[0]: EVENTS.append({"kind": "input", "t0": commits[0]["t0"], "t1": commits[0]["t1"], "tag": "pan_during_commits"})
    time.sleep(1)
# 5. cross-probe: Backplane -> KiCad (editor.kicad.select, what an agent or a chat ref does)
if "xprobe" in JOURNEYS:
    proj = KPRO or geo.get("project") or ""
    refs = []
    if PCB and os.path.exists(PCB): refs = re.findall(r'\(property "Reference" "([A-Z]+[0-9]+)"', open(PCB, errors="ignore").read())[:40:8]
    for i, ref in enumerate(refs[:4]):
        t0 = now(); out = rpc([{"method": "editor.kicad.select", "params": {"project": proj, "references": [ref], "zoom": True}}]); t1 = now()
        # timed from the request itself (hub_rpc.mjs sent_at / replied_at): the helper's start, connect and 250 ms settle are not the probe's
        r = jl(out) or {}; sent = r.get("sent_at"); got = r.get("replied_at")
        EVENTS.append({"kind": "switch", "t0": sent or t0, "tag": f"xprobe{i}_{ref}", "window_ms": 2000})
        perf(f"xprobe_rpc_{i}", round(got - sent) if sent and got else round(t1 - t0), ref=ref, ok=int('"ok":true' in out.replace(" ", "")), helper_ms=round(t1 - t0))
        time.sleep(2)
# 7. save: Ctrl+S to the file written and KiCad clean; KiCad's ping latency through it
if "save" in JOURNEYS and SOCK and PCB:
    tab("PCB", "save_board", 1.0)
    ipc("agent-add-track", SOCK, 120, 120, 130, 120, 0.25); time.sleep(0.5)
    rt("click", CX, PANE["y"] + PANE["h"] - 40); time.sleep(0.3)
    m0 = os.stat(PCB).st_mtime; r = rt("key", "s", "ctrl"); t0 = r["t0"]; written = clean = None
    while now() - t0 < 30000:
        if written is None and os.stat(PCB).st_mtime > m0: written = now()
        if written and ipc("modified", SOCK, "pcb").get("modified") is False: clean = now(); break
        time.sleep(0.02)
    perf("save_written", round(written - t0) if written else -1); perf("save_clean", round(clean - t0) if clean else -1)
# 7b. tools: KiCad's own DRC, Plot (Gerbers) and ERC dialogs as the person runs them (the dialog open, Return on its default button, how long
# KiCad stays busy: its API ping answered slowly), then the same checks and exports by kicad-cli (what agents and the hub run) while the person pans
def busy_after(t0, secs):
    """KiCad's API ping every 100 ms for SECS: the slowest answer, and when the last slow (> 100 ms) one ended (busy time from t0)."""
    pings, last_slow = [], None
    while now() - t0 < secs * 1000:
        a = now(); ok = ipc("ping", SOCK, timeout=30); b = now(); pings.append(b - a)
        if b - a > 100: last_slow = b
        time.sleep(0.1)
    return (round(max(pings)) if pings else None), (round(last_slow - t0) if last_slow else 0)
def find_win(title_part, secs):
    t = time.time()
    while time.time() - t < secs:
        for w in windows():
            if w.get("shown") and title_part.lower() in w.get("title", "").lower() and "kicad" in w.get("wm_class", "").lower(): return w
        time.sleep(0.03)
    return None
def dialog_run(action, title, tag, run_secs=20, press_return=True):
    t0 = now(); call = subprocess.Popen(["python3", IPC, "run-action", SOCK, action], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    w = find_win(title, 20)
    if not w: perf(f"{tag}_dialog_open", -1, found=0); call.kill(); return
    perf(f"{tag}_dialog_open", round(now() - t0), found=1); shot(f"{tag}_dialog")
    sh("python3", XIN, "focus", w["title"]); time.sleep(0.2)
    if not press_return:  # the Plot dialog's Return lands on its format list (a menu), not on Plot: only its opening is measured
        sh("python3", XIN, "focus", w["title"]); rt("key", "Escape"); time.sleep(0.5)
        try: call.wait(timeout=10)
        except Exception: call.kill()
        return
    t1 = rt("key", "Return").get("t0")
    worst, busy = busy_after(t1, run_secs)
    perf(f"{tag}_run_busy", busy, ping_max=worst); shot(f"{tag}_done")
    sh("python3", XIN, "focus", w["title"]); rt("key", "Escape"); time.sleep(0.5)
    if find_win(title, 0.3): rt("key", "Escape"); time.sleep(0.5)
    try: call.wait(timeout=10)
    except Exception: call.kill()
if "tools" in JOURNEYS and SOCK:
    PAUSE.set()
    tab("PCB", "tools_board", 1.2); rt("click", CX, CY + PANE["h"] // 3); time.sleep(0.4)
    dialog_run("pcbnew.DRCTool.runDRC", "Design Rules Checker", "drc_gui")
    gerb0 = set(glob.glob(os.path.join(PROJ_DIR, "**", "*.g*"), recursive=True))
    dialog_run("common.Control.plot", "Plot", "plot_gui", press_return=False)
    tab("SCH", "tools_sch", 1.5); rt("click", CX, PANE["y"] + PANE["h"] - 40); time.sleep(0.4)
    dialog_run("eeschema.InspectionTool.runERC", "Electrical Rules Checker", "erc_gui")
    tab("PCB", "tools_back", 1.2)
    PAUSE.clear()
    cli = os.environ.get("KICAD_CLI") or ""
    sch = PCB[:-len(".kicad_pcb")] + ".kicad_sch"
    jobs = [("drc_cli", [cli, "pcb", "drc", "--format", "json", "-o", os.path.join(OUT, "drc.json"), PCB]),
            ("erc_cli", [cli, "sch", "erc", "--format", "json", "-o", os.path.join(OUT, "erc.json"), sch]),
            ("gerbers_cli", [cli, "pcb", "export", "gerbers", "-o", os.path.join(OUT, "gerbers") + "/", PCB])]
    for tag, cmd in jobs:
        if not os.path.exists(cli): break
        os.makedirs(os.path.join(OUT, "gerbers"), exist_ok=True)
        box = {}
        th = threading.Thread(target=lambda: box.update(r=rt("pan", CX, CY, min(PANE["w"], PANE["h"]) // 5, 5, 2, 120))); th.start(); time.sleep(0.3)
        t0 = now(); rc = subprocess.run(cmd, capture_output=True, timeout=300).returncode; t1 = now()
        th.join(); perf(tag, round(t1 - t0), rc=rc)
        if box.get("r"): EVENTS.append({"kind": "input", "t0": box["r"]["t0"], "t1": box["r"]["t1"], "tag": f"pan_during_{tag}"})
        time.sleep(0.5)
def fp_positions():
    txt = open(PCB, errors="ignore").read(); out = {}
    for m in re.finditer(r'\(footprint "[^"]*"(.*?)\n\t\)', txt, re.S):
        body = m.group(1); ref = re.search(r'\(property "Reference" "([^"]+)"', body)
        at = re.search(r'\n\t\t\(at ([-\d.]+) ([-\d.]+)', body) or re.search(r'\n\t\t\(transform\s*\(translate ([-\d.]+) ([-\d.]+)\)', body)  # 10.0 / 10.99 forms
        if at and ref: out[ref.group(1)] = (float(at.group(1)), float(at.group(2)))
    return out
# 7a. textkeys (bp.13 batch 4: comment-key-handoff, embed-popup-keys): ';' on the canvas opens Backplane's comment window; ';' typed in a
# KiCad text field (the search panel's box, a properties field) stays there and opens nothing; Backplane's chords (Ctrl+K, Ctrl+;) while a
# KiCad right-click menu is open reach Backplane.
def popups(): return [w for w in windows() if w.get("shown") and w.get("override_redirect") and 60 < w["w"] < 1200 and 60 < w["h"] < 900]
def comment_after(keys, secs=1.5):
    fo = xfocus(); kf = {k: f.get("focused") for k, f in kframes().items()}
    print(f"INFO textkeys focus before {'+'.join(keys[::-1])}: X {fo[:2]}; KiCad frames focused {kf}", flush=True)
    n = len(logs("kcomment", "painted")); r = rt("key", *keys); got = wait_log("kcomment", "painted", n, secs)
    return r, got
def palette_after(keys):
    box = {"x": 600, "y": 0, "w": 700, "h": 420}
    sampler_stop(); sampler_start(box, "tk_palette"); r = rt("key", *keys); time.sleep(1.2); kp = sampler_stop(); sampler_start(PANE, "pane")
    a = analyze([{"kind": "commit", "t0": r["t0"], "tag": "tk", "window_ms": 1000}], kp)
    return a[0].get("first_change_ms") if a else None
if "textkeys" in JOURNEYS and SOCK:
    from pane_sampler import grab
    PAUSE.set()
    EXk, EYk = CX, PANE["y"] + PANE["h"] - 40
    tab("PCB", "tk_board", 1.2); rt("click", EXk, EYk); time.sleep(0.4); rt("key", "Escape"); time.sleep(0.3)
    # (a) ';' on the canvas: the comment key handed to Backplane
    r, got = comment_after(("semicolon",))
    perf("semicolon_canvas", round(got["at_ms"] - r["t0"], 1) if got and got.get("at_ms") else -1, opened=int(bool(got)))
    if got: time.sleep(0.4); rt("key", "Escape"); time.sleep(0.6)
    # (b) the search panel (Ctrl+G): ';' typed in its box stays there
    rt("click", EXk, EYk); time.sleep(0.3)
    g0 = grab(PANE["x"], PANE["y"], PANE["w"], PANE["h"]); rt("key", "g", "ctrl"); time.sleep(1.2); shot("tk_search_open")
    # a person types in the box: click into it (the search panel docks at the bottom, its box ~143 px above the pane's bottom at 1800x1050)
    rt("click", PANE["x"] + PANE["w"] // 3, PANE["y"] + PANE["h"] - 143); time.sleep(0.5)
    r, got = comment_after(("semicolon",)); time.sleep(0.3); shot("tk_semicolon_in_search")
    RESULTS["semicolon_in_search_stays"] = {"ms": None, "ok": int(not got)}
    print(f"INFO textkeys: ';' in the search box {'opened the comment window' if got else 'stayed in the box'}", flush=True)
    if got: time.sleep(0.3); rt("key", "Escape"); time.sleep(0.5)
    rt("key", "Escape"); time.sleep(0.4); rt("key", "g", "ctrl"); time.sleep(0.8)  # the search panel shut again
    # (c) the properties panel: shown (KiCad's action), a footprint selected, the panel found by the pixels it changes, a click in its
    # upper field area, ';'
    refs = [r_ for r_ in fp_positions() if re.match(r"^[RC]\d+$", r_)] or list(fp_positions())
    if refs:
        rt("click", EXk, EYk); time.sleep(0.3); ipc("select-items", SOCK, "pcb", refs[0], "0"); time.sleep(0.5)
        ga = grab(PANE["x"], PANE["y"], PANE["w"], PANE["h"]); ipc("run-action", SOCK, "common.Control.showProperties"); time.sleep(1.5)
        gb = grab(PANE["x"], PANE["y"], PANE["w"], PANE["h"]); shot("tk_properties")
        import numpy as np
        # the panel docks at the pane's left; its Reference value field ~ (pane+154, pane+396) at 1800x1050 (measured)
        changed = float((ga[:, :240] != gb[:, :240]).mean()) > 0.3
        if changed:
            fx, fy = PANE["x"] + 154, PANE["y"] + 396
            rt("click", fx, fy); time.sleep(0.5); rt("click", fx, fy); time.sleep(0.4); shot("tk_properties_field")
            rt("key", "End"); time.sleep(0.2)
            r, got = comment_after(("semicolon",)); time.sleep(0.3); shot("tk_semicolon_in_properties")
            RESULTS["semicolon_in_properties_stays"] = {"ms": None, "ok": int(not got), "at": [fx, fy]}
            print(f"INFO textkeys: ';' in the properties Reference field at {fx},{fy} {'opened the comment window' if got else 'stayed in the field'}", flush=True)
            if got: time.sleep(0.3); rt("key", "Escape"); time.sleep(0.5)
            rt("key", "BackSpace"); time.sleep(0.2); rt("key", "Escape"); time.sleep(0.3)  # the ';' out again, the edit dropped
        else:
            print("DIAG textkeys: the properties panel did not appear at the pane's left", flush=True)
        ipc("run-action", SOCK, "common.Control.showProperties"); time.sleep(0.8)
    # (d) Backplane's chords with KiCad's right-click menu open
    for keys, tag in ((("k", "ctrl"), "popup_ctrl_k"), (("semicolon", "ctrl"), "popup_ctrl_semicolon")):
        rt("click", EXk, EYk); time.sleep(0.3); rt("click", EXk, EYk - 60, 3); time.sleep(0.8)
        m = popups() or [w for w in windows() if w.get("shown") and w.get("override_redirect") and w["w"] > 40 and w["h"] > 40 and w["w"] < 1500]
        shot(f"tk_{tag}_menu")
        if not m: print(f"DIAG textkeys: the right-click opened no menu ({tag})", flush=True); continue
        if tag == "popup_ctrl_k":
            ck = palette_after(keys); perf(tag, ck if ck is not None else -1, opened=int(ck is not None), menu_open=1)
        else:
            r, got = comment_after(keys); perf(tag, round(got["at_ms"] - r["t0"], 1) if got and got.get("at_ms") else -1, opened=int(bool(got)), menu_open=1)
        RESULTS[f"{tag}_menu_after"] = {"ms": None, "ok": int(not popups())}
        rt("key", "Escape"); time.sleep(0.4); rt("key", "Escape"); time.sleep(0.4)
        if popups(): rt("key", "Escape"); time.sleep(0.4)
    PAUSE.clear()
# 7b. editors (bp.13 batch 2 on): the footprint editor opened from the board as a person does (a footprint clicked, Ctrl+E), then KiCad's
# own footprint-editor action; GerbView in KiCad's process on the board's Gerbers (OpenGerbers: hidden as a prewarm, then shown over the
# pane, as a host would; the PCB editor has no GerbView command of its own). What appears where, how long it takes, and what the client shows.
FK = {1: "PCB", 2: "SCH", 3: "SYM", 4: "FP", 5: "PCB_3D", 6: "OTHER", 7: "GERBVIEW"}
ET_NAMES = {9: "ET_SHOW_REQUESTED", 25: "ET_EDITOR_REQUESTED"}
def kframes(): return {FK.get(f["kind"], str(f["kind"])): f for f in ipc("frames", SOCK).get("frames", [])}
def kicad_tops():  # KiCad top-level windows a person can see (not the pane's own frame): a floating editor, a dialog
    return [w for w in windows() if w.get("shown") and "kicad" in w.get("wm_class", "").lower() and w["w"] > 100 and w["h"] > 100
            and not (abs(w["root_x"] - PANE["x"]) < 40 and abs(w["root_y"] - PANE["y"]) < 60)
            and w["root_x"] + w["w"] > 0 and w["root_y"] + w["h"] > 0]  # not a prewarmed frame laid out off-screen
def editor_watch(tag, kind, t0, secs=20):
    """After an action at t0: when KiCad's KIND frame is shown, the events KiCad posted, any new top-level KiCad window, what the
    client last reported shown, and screenshots at 1 s and at the end."""
    tops0 = {w["xid"] for w in kicad_tops()}; shown_ms = None; newtops = []; shot1 = False
    while now() - t0 < secs * 1000:
        f = kframes().get(kind) or {}
        if f.get("shown") and shown_ms is None: shown_ms = round(now() - t0)
        for w in kicad_tops():
            if w["xid"] not in tops0 and w["xid"] not in [n["xid"] for n in newtops]:
                newtops.append({"xid": w["xid"], "title": w.get("title"), "at": [w["root_x"], w["root_y"], w["w"], w["h"]], "ms": round(now() - t0)})
        if not shot1 and now() - t0 > 1000: shot(f"{tag}_1s"); shot1 = True
        if shown_ms is not None and now() - t0 > shown_ms + 3000: break
        time.sleep(0.1)
    shot(f"{tag}_end")
    f = kframes().get(kind) or {}
    sn = (metrics("shown") or [{}])[-1]
    out = {"ms": shown_ms, "shown": int(bool(f.get("shown"))), "lib_id": f.get("lib_id"), "title": f.get("title"), "files": len(f.get("files") or []),
           "new_windows": newtops, "client_shown": {k: sn.get(k) for k in ("thread", "open", "active", "kind", "tab") if k in sn},
           "pane_editor_up": int(editor_up())}
    RESULTS[tag] = out
    print(f"PERF {tag} {shown_ms}ms " + " ".join(f"{k}={json.dumps(v)}" for k, v in out.items() if k != "ms"), flush=True)
    for w in newtops: print(f"JANK {tag}: a KiCad window outside the pane: {w['title']!r} at {w['at']} after {w['ms']} ms", flush=True)
    return out
CAPS = ipc("capabilities", SOCK) if SOCK else {}
print(f"INFO kicad capabilities: {sorted(CAPS.get('capabilities', []))}", flush=True)
RESULTS["kicad_capabilities"] = {"ms": None, "caps": sorted(CAPS.get("capabilities", [])), "build": KICAD_RUN.get("exe"), "commit": KICAD_RUN.get("backplane_commit")}
def has_cap(cap, command):  # batch 3 on names it in capabilities; 77a6ff17d0 (batch 2) only registers the command
    return cap in CAPS.get("capabilities", []) or command in CAPS.get("commands", [])
if "editors" in JOURNEYS and SOCK and not has_cap("embed-gerbview", "kiapi.backplane.embed.OpenGerbers"):
    print("DIAG editors: this runtime has no in-process editors (no embed-gerbview / OpenGerbers): skipped", flush=True)
    RESULTS["editors_supported"] = {"ms": None, "ok": 0}
elif "editors" in JOURNEYS and SOCK:
    PAUSE.set(); RESULTS["editors_supported"] = {"ms": None, "ok": 1}
    fr0 = kframes(); print("INFO editors frames before: " + json.dumps({k: [f["shown"], f["lib_id"]] for k, f in fr0.items()}), flush=True)
    ev0 = ipc("events", SOCK, "0").get("next", 0)
    tab("PCB", "editors_board", 1.2)
    refs = sorted(fp_positions(), key=lambda r: (0 if r.startswith("U") else 1, len(r), r))
    ref = refs[0] if refs else ""
    sel = ipc("select-items", SOCK, "pcb", ref, "1"); time.sleep(1.0)
    rt("click", CX, CY); time.sleep(0.5)  # the footprint, centred by the zoom, clicked: the canvas has the keyboard and the footprint is selected
    sl = ipc("selection", SOCK, "pcb")
    if not sl.get("count") or not any("FootprintInstance" in t for t in sl.get("types", [])):
        # the click hit a pad (a module's pads cover its middle) or nothing: the footprint itself, selected again (the keyboard stays in the canvas)
        rt("key", "Escape"); time.sleep(0.3); sel = ipc("select-items", SOCK, "pcb", ref, "0"); time.sleep(0.5)
    print(f"INFO editors: {ref} selected over the API ({sel.get('found')}), clicked; selection {ipc('selection', SOCK, 'pcb')}", flush=True)
    r = rt("key", "e", "ctrl")
    a = editor_watch("editor_fp_ctrl_e", "FP", r["t0"])
    evs = ipc("events", SOCK, str(ev0)).get("events", [])
    got = [{"type": ET_NAMES.get(e["type"], e["type"]), "kind": FK.get(e["frame"].get("kind")), "lib_id": e["frame"].get("lib_id")}
           for e in evs if e["type"] in ET_NAMES]
    RESULTS["editor_fp_ctrl_e"]["events"] = got
    print(f"INFO editors: KiCad's events after Ctrl+E: {got}", flush=True)
    RESULTS["editor_fp_from_board_ok"] = {"ms": None, "ok": int(bool(a["shown"] and a["lib_id"] and not a["new_windows"]))}
    tab("PCB", "editors_back1", 1.5); shot("editors_back1")
    RESULTS["editor_fp_back_to_board"] = {"ms": None, "ok": int(editor_up() and not (kframes().get("FP") or {}).get("shown"))}
    # KiCad's own command (the PCB editor's toolbar button "Footprint Editor")
    t0 = now(); st = ipc("run-action", SOCK, "common.Control.showFootprintEditor")
    b = editor_watch("editor_fp_action", "FP", t0); RESULTS["editor_fp_action"]["action_ok"] = int(bool(st.get("ok")))
    tab("PCB", "editors_back2", 1.5)
    # GerbView: the board's Gerbers and drill files from kicad-cli, as Backplane's mfg/ holds them
    cli = os.environ.get("KICAD_CLI") or ""; mfg = os.path.join(OUT, "mfg"); os.makedirs(mfg, exist_ok=True)
    if os.path.exists(cli) and PCB:
        subprocess.run([cli, "pcb", "export", "gerbers", "-o", mfg + "/", PCB], capture_output=True, timeout=300)
        subprocess.run([cli, "pcb", "export", "drill", "-o", mfg + "/", PCB], capture_output=True, timeout=300)
    files = sorted(f for f in glob.glob(os.path.join(mfg, "*")) if re.search(r"\.(g[a-z0-9]{1,3}|drl|gbr)$", f, re.I) and not f.endswith(".gbrjob"))
    if not files: print("DIAG editors: no Gerbers exported", flush=True)
    else:
        tops0 = {w["xid"] for w in kicad_tops()}
        t0 = now(); g = ipc("open-gerbers", SOCK, ",".join(files), timeout=120); ms = round(now() - t0)
        newt = [w for w in kicad_tops() if w["xid"] not in tops0]
        RESULTS["gerbview_prewarm"] = {"ms": ms, "loaded": len(g.get("loaded", [])), "of": len(files), "failed": dict(zip(g.get("failed", []), g.get("errors", []))),
                                       "shown": int(bool((g.get("frame") or {}).get("shown"))), "new_windows": [w.get("title") for w in newt], "ok": int(bool(g.get("ok")))}
        print(f"PERF gerbview_prewarm {ms}ms " + json.dumps(RESULTS["gerbview_prewarm"]), flush=True)
        shot("gerbview_prewarm")
        evg = ipc("events", SOCK, "0").get("next", 0)
        # shown the way a person gets it: through the hub (editor.kicad.show GERBVIEW with the board's job file, as the client's
        # open_gerbers() and the Checks panel's "Open in GerbView" do), so the hub and the client know GerbView is the active editor
        job = next((f for f in glob.glob(os.path.join(mfg, "*.gbrjob"))), "")
        if job:
            t0 = now(); out = rpc([{"method": "editor.kicad.show", "params": {"project": KPRO, "frame": "GERBVIEW", "document": job}}])
            r = jl(out) or {}; sent = r.get("sent_at"); got = r.get("replied_at")
            if sent: t0 = sent
            call_ms = round((got or now()) - t0); g2 = {"loaded": files}
            print(f"INFO editors: editor.kicad.show GERBVIEW ({os.path.basename(job)}) answered in {call_ms} ms: {out.strip()[-160:]}", flush=True)
        else:  # no job file: KiCad's own OpenGerbers, which the hub does not see (the client will hide it)
            t0 = now(); g2 = ipc("open-gerbers", SOCK, ",".join(files), "0", "0", str(PANE["w"]), str(PANE["h"]), timeout=120); call_ms = round(now() - t0)
            print(f"INFO editors: no .gbrjob; OpenGerbers shown over the API (the hub is not told) in {call_ms} ms", flush=True)
        RESULTS["gerbview_via_hub"] = {"ms": None, "ok": int(bool(job))}
        shot("gerbview_call_returned")
        # who has the pane for the next 5 s: each frame's shown flag every 100 ms, its changes, and KiCad's events
        trace, last = [], None
        while now() - t0 < call_ms + 5000:
            st = tuple(sorted(k for k, f in kframes().items() if f["shown"]))
            if st != last: trace.append([round(now() - t0), list(st)]); last = st
            time.sleep(0.1)
        evs = [{"seq": e["seq"], "type": e["type"], "kind": FK.get(e["frame"].get("kind")), "detail": e.get("detail", "")[:60]}
               for e in ipc("events", SOCK, str(evg)).get("events", [])]
        print(f"INFO editors: frames shown after OpenGerbers(show): {trace}", flush=True)
        print(f"INFO editors: KiCad's events after OpenGerbers(show): {evs[:30]}", flush=True)
        c = editor_watch("gerbview_shown", "GERBVIEW", t0, secs=0)
        RESULTS["gerbview_shown"]["ms"] = next((t for t, k in trace if "GERBVIEW" in k), None)
        RESULTS["gerbview_shown"]["stays"] = int(bool(trace) and "GERBVIEW" in trace[-1][1])
        RESULTS["gerbview_shown"].update(call_ms=call_ms, loaded=len(g2.get("loaded", [])), trace=trace, events=evs[:30])
        ipc("hide-frame", SOCK, "gerbview"); time.sleep(0.8)
        tab("PCB", "editors_back3", 1.5); shot("editors_back3")
        RESULTS["gerbview_back_to_board"] = {"ms": None, "ok": int(editor_up() and not (kframes().get("GERBVIEW") or {}).get("shown"))}
    print("INFO editors frames after: " + json.dumps({k: [f["shown"], f["lib_id"], len(f["files"])] for k, f in kframes().items()}), flush=True)
    PAUSE.clear()
# 7c. plan: the routing floorplanner (fork bp.13+, a <board>.kicad_plan beside the board): plan view on/off (Alt+P), the plan layer, a corridor
# vertex and a plane vertex dragged 5 s each (frame times), W / Shift+W, A / R, and an agent's ProposePlan to the overlay on screen.
# Board millimetres are put on screen by a two-point calibration: two footprints selected over the API, each found by the pixels that change.
PLANP = os.path.join(os.path.dirname(os.path.abspath(__file__)), "plan_probe.py")
def plan(*a, timeout=30): return jl(sh("python3", PLANP, *a, timeout=timeout))
def changed_centroid(before, after, min_px=20):
    import numpy as np
    ys, xs = np.nonzero(before != after)
    if len(xs) < min_px: return None
    return float(xs.mean()), float(ys.mean())
def calibrate():
    from pane_sampler import grab
    # small two-pad parts (C, R): their origin is their middle, which is where a selection's changed pixels centre
    fps = fp_positions(); refs = [r for r in fps if re.match(r"^[CR]\d+$", r)] or [r for r in fps if re.match(r"^[A-Z]+\d+$", r)]
    if len(refs) < 2: print(f"DIAG plan calibration: {len(refs)} footprints parsed", flush=True); return None
    # two far-apart footprints
    ranked = sorted(refs, key=lambda r: fps[r][0] + fps[r][1])
    pts = []; used = []
    for ref in ranked[:6] + ranked[::-1][:6]:  # from each end, the first two (one per end) whose selection shows
        if len(pts) == 2: break
        if len(pts) == 1 and (ref in used or ranked.index(ref) < len(ranked) // 2): continue
        # the canvas only (not the toolbars, the header or the status bar, which a selection changes too): measured at 1800x1050
        CV = (PANE["x"] + 40, PANE["y"] + 110, 720, 715)
        rt("key", "Escape"); time.sleep(0.4); g0 = grab(*CV)
        ipc("select-items", SOCK, "pcb", ref, 0); time.sleep(0.6); g1 = grab(*CV)
        c = changed_centroid(g0, g1)
        if c: c = (c[0] + 40, c[1] + 110)
        if not c: print(f"DIAG plan calibration: selecting {ref} changed no pixels; the next one", flush=True); continue
        pts.append((fps[ref], (PANE["x"] + c[0], PANE["y"] + c[1]))); used.append(ref)
    if len(pts) < 2: print("DIAG plan calibration: fewer than two footprints showed their selection", flush=True); return None
    a, b = used
    rt("key", "Escape"); time.sleep(0.3)
    (b1, s1), (b2, s2) = pts
    k = ((s2[0] - s1[0]) / (b2[0] - b1[0]) + (s2[1] - s1[1]) / (b2[1] - b1[1])) / 2 if b2[0] != b1[0] and b2[1] != b1[1] else None
    if not k or k <= 0: print(f"DIAG plan calibration: bad scale from {pts}", flush=True); return None
    ox, oy = s1[0] - k * b1[0], s1[1] - k * b1[1]
    print(f"INFO plan calibration: {a}@{s1} {b}@{s2} -> {k:.3f} px/mm", flush=True)
    return lambda x, y: (ox + k * x, oy + k * y)
if "plan" in JOURNEYS and SOCK:
    got = plan("get", SOCK); pj = {}
    try: pj = json.loads(got.get("json") or "{}")
    except Exception: pass
    if not pj.get("corridors"):
        print(f"DIAG plan: no plan on this board/runtime ({str(got)[:120]})", flush=True)
    else:
        from pane_sampler import grab
        import numpy as np
        CV = (PANE["x"] + 40, PANE["y"] + 110, PANE["w"] - 80, PANE["h"] - 180)  # the canvas (not the toolbars or the status bar)
        TB = (PANE["x"] + 4, PANE["y"] + 100, 28, PANE["h"] - 170)                 # the left toolbar's icons (the canvas starts at +34)
        def frac(a_, b_): return float((a_ != b_).mean())
        def bbox(a_, b_):
            ys, xs = np.nonzero(a_ != b_)
            return None if len(xs) < 8 else (int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max()))
        def menus():  # a popup menu: override-redirect, smaller than the screen (the client's own full-screen surfaces are not menus)
            return [w for w in windows() if w.get("shown") and w.get("override_redirect") and 60 < w["w"] < 1200 and 60 < w["h"] < 900]
        def action_toggle(): t = now(); ipc("run-action", SOCK, "pcbnew.Plan.togglePlanView"); return t
        bc = (KICAD_RUN.get("backplane_commit") or "")[:10]
        ALT_P_BUILDS = ("d5eaa3ae44", "3de539d24a", "77a6ff17d0")  # bp.13 batches 1-2: Alt+P; batch 3 (c53b5e0800) on: Ctrl+Shift+R
        PLAN_KEY = os.environ.get("KEMBED_PLAN_KEY") or ("alt+p" if bc in ALT_P_BUILDS else "ctrl+shift+r")
        def press_plan_key():
            mods = PLAN_KEY.split("+"); return rt("key", mods[-1], *mods[:-1])["t0"]
        STATE = {"on": False}  # a fresh profile opens with the plan view off
        def toggled(tag, how, settle=1.2):
            """One toggle by HOW (a function returning t0): the canvas changed or not, the switch's frames measured, the state followed."""
            g0 = grab(*CV); t0 = how(); time.sleep(settle); g1 = grab(*CV)
            ch = frac(g0, g1) > 0.01; m = menus()
            if ch: STATE["on"] = not STATE["on"]
            EVENTS.append({"kind": "switch", "t0": t0, "tag": tag, "window_ms": int(settle * 1000)})
            RESULTS[f"{tag}_toggled"] = {"ms": None, "ok": int(ch and not m)}
            if m: print(f"JANK {tag}: a menu opened ({m[0]['w']}x{m[0]['h']})", flush=True); shot(f"{tag}_menu"); rt("key", "Escape"); time.sleep(0.4)
            return ch
        # A. the person's key with no click on the canvas: (a) straight after Edit (the last click was Backplane's Edit button: what a person
        # does), (b) after the pane's Full size (lane J's embed_plan_person_e2e), (c) after a click on the Board tab. Where X's keyboard focus
        # is each time (the window and its parents) and whether KiCad says its frame has the keyboard (FrameInfo.focused).
        def focus_now(tag):
            fo = xfocus(); kf = (kframes().get("PCB") or {})
            RESULTS[f"{tag}_focus"] = {"ms": None, "x": fo, "kicad_frame_focused": kf.get("focused")}
            print(f"INFO focus {tag}: X focus {fo}; KiCad PCB frame focused={kf.get('focused')}", flush=True)
        for case, prep in (("after_edit", None), ("after_full_size", "kicad:full"), ("after_board_tab", "kicad-tab:PCB")):
            if prep:
                click_ctl(prep); time.sleep(1.5)
                if prep == "kicad:full": click_ctl(prep); time.sleep(1.5)  # Full size on, then off again: the pane as it was
            focus_now(f"plan_key_{case}")
            ok = toggled(f"plan_key_{case}", press_plan_key)
            RESULTS[f"plan_key_{case}_ok"] = {"ms": None, "ok": int(ok)}
            print(f"INFO plan view by {PLAN_KEY} {case.replace('_', ' ')}, no click on the canvas: {'toggled' if ok else 'nothing'} (BACKPLANE_COMMIT {bc})", flush=True)
            shot(f"plan_key_{case}")
            if STATE["on"]: toggled(f"plan_off_{case}", action_toggle)
        RESULTS["plan_key_without_click_ok"] = {"ms": None, "ok": RESULTS["plan_key_after_edit_ok"]["ok"]}
        rt("click", CX, PANE["y"] + PANE["h"] - 30); time.sleep(0.3); rt("key", "Home"); time.sleep(1.0)
        to_screen = calibrate()
        RESULTS["plan_calibrated"] = {"ms": None, "ok": int(to_screen is not None)}
        # B. the left toolbar's button, found by its checked state: the toolbar before and after a RunAction toggle
        t_a = grab(*TB); act = toggled("plan_action_probe", action_toggle); t_b = grab(*TB)
        RESULTS["plan_toggle_by_action"] = {"ms": None, "ok": int(act)}
        btn = None
        rows = (t_a != t_b).sum(axis=1); segs = []; start = None
        for y_, v in enumerate(list(rows) + [0]):
            if v and start is None: start = y_
            if not v and start is not None: segs.append((start, y_ - 1, int(rows[start:y_].sum()))); start = None
        segs = [g for g in segs if g[1] - g[0] <= 40]
        if segs:
            g = max(segs, key=lambda g: g[2]); btn = (0, g[0], TB[2] - 1, g[1])
        print(f"INFO plan toolbar: changed bands {segs[:8]}", flush=True)
        print(f"INFO plan view by RunAction: {'toggled' if act else 'nothing'}; toolbar button {btn}", flush=True)
        if btn:
            bx, by = TB[0] + (btn[0] + btn[2]) // 2, TB[1] + (btn[1] + btn[3]) // 2
            for i in range(10):
                toggled(f"plan_toolbar{i:02d}_{'on' if not STATE['on'] else 'off'}", lambda: rt("click", bx, by)["t0"])
        else: print("DIAG plan: the left toolbar shows no checked plan-view button", flush=True)
        RESULTS["plan_toolbar_found"] = {"ms": None, "ok": int(bool(btn))}
        # C. the View menu's item, found the same way: the menu open with the view off and on
        VIEW_AT = (PANE["x"] + 97, PANE["y"] + 14)  # the menu bar's "View", measured at 1800x1050
        def open_view():
            rt("click", *VIEW_AT); time.sleep(0.7); m = menus(); return m[0] if m else None
        m1 = open_view(); item = None
        if m1:
            g_1 = grab(m1["root_x"], m1["root_y"], m1["w"], m1["h"]); shot("plan_view_menu"); rt("key", "Escape"); time.sleep(0.4)
            toggled("plan_menu_probe", action_toggle)
            m2 = open_view()
            if m2 and (m2["w"], m2["h"]) == (m1["w"], m1["h"]):
                g_2 = grab(m2["root_x"], m2["root_y"], m2["w"], m2["h"]); bb = bbox(g_1, g_2)
                if bb: item = (m2["root_x"] + m2["w"] // 2, m2["root_y"] + (bb[1] + bb[3]) // 2)
            rt("key", "Escape"); time.sleep(0.4)
        if not item and m1:  # no checkmark difference found: the row as measured at 1800x1050 (95 px below the menu's top)
            item = (m1["root_x"] + 100, m1["root_y"] + 95)
        print(f"INFO plan view menu: {'open' if m1 else 'did not open'}; item at {item}", flush=True)
        menu_ok = []
        if item:
            for i in range(2):
                def by_menu():
                    open_view(); return rt("click", *item)["t0"]
                menu_ok.append(toggled(f"plan_menu{i}_{'on' if not STATE['on'] else 'off'}", by_menu))
        RESULTS["plan_menu_found"] = {"ms": None, "ok": int(bool(item) and any(menu_ok))}
        # D. the key with the canvas holding the keyboard (a click on an empty corner first)
        def key_after_click():
            rt("click", PANE["x"] + 60, PANE["y"] + PANE["h"] - 140); time.sleep(0.2); return press_plan_key()
        for i in range(10):
            toggled(f"plan_toggle{i:02d}_{'on' if not STATE['on'] else 'off'}", key_after_click)
        # the rest of the journey works in the plan view
        if not STATE["on"]: toggled("plan_view_on", action_toggle, 2.0)
        if not STATE["on"]: toggled("plan_view_on2", key_after_click, 2.0)
        RESULTS["plan_view_reached"] = {"ms": None, "ok": int(STATE["on"])}
        if STATE["on"]:  # entering plan view refits the view (PLAN_TOOL::fitBesideOverview): the mapping measured before it is stale
            t2 = calibrate()
            if t2: to_screen = t2; print("INFO plan: recalibrated in plan view", flush=True)
            else: print("DIAG plan: no calibration in plan view; the one from before it is used (clicks may miss)", flush=True)
            RESULTS["plan_calibrated_in_plan_view"] = {"ms": None, "ok": int(bool(t2))}
        shot("plan_view")
        cor = pj["corridors"][0]; cl = cor["layer"]
        order = sorted({l["layer"] for l in pj.get("layers", [])}, key=lambda n: 0 if n == "F.Cu" else (99 if n == "B.Cu" else int(re.sub(r"\D", "", n) or 50)))
        def panel_click(layer, tag):  # the plan view's mini panel for LAYER, in stack order (L1 = F.Cu)
            i = order.index(layer) if layer in order else 0
            if len(order) <= 4: px, py = PANE["x"] + 708, PANE["y"] + 179 + 120 * i            # one column (measured at 1800x1050)
            else: px, py = PANE["x"] + 679 + 52 * (i % 2), PANE["y"] + 185 + 135 * (i // 2)  # two columns: L1 L2 / L3 L4 ... (Hubble, 8 layers)
            before = plan("layer", SOCK).get("layer")
            r = rt("click", px, py)
            EVENTS.append({"kind": "switch", "t0": r["t0"], "tag": tag, "window_ms": 1500}); time.sleep(1.5)
            print(f"INFO plan layer after the {layer} panel (L{i + 1} at {px},{py}): {before} -> {plan('layer', SOCK).get('layer')}", flush=True)
        panel_click(cl, "plan_layer_corridor"); shot("plan_layer_corridor")
        if to_screen:
            path = cor["path"]; mid = ((path[0][0] + path[1][0]) / 2, (path[0][1] + path[1][1]) / 2)
            ly0 = plan("layer", SOCK).get("layer")
            mx, my = to_screen(*mid); rt("click", mx, my); time.sleep(0.6); shot("plan_corridor_selected")
            ly1 = plan("layer", SOCK).get("layer")
            selp = ipc("selection", SOCK, "pcb")
            RESULTS["plan_corridor_click"] = {"ms": None, "ok": int(ly1 == cl), "board_mm": [round(mid[0], 3), round(mid[1], 3)], "screen": [round(mx), round(my)],
                                              "layer_before": ly0, "layer_after": ly1, "selection": selp.get("types")}
            p0s, p1s = to_screen(*path[0]), to_screen(*path[1])
            print(f"INFO plan corridor click: {cor['id']} on {cl} at board {mid[0]:.3f},{mid[1]:.3f} mm (screen {mx:.0f},{my:.0f}; the segment runs "
                  f"{p0s[0]:.0f},{p0s[1]:.0f} -> {p1s[0]:.0f},{p1s[1]:.0f}); active layer {ly0} -> {ly1}; selection {selp.get('types')}", flush=True)
            sn = (metrics("shown") or [{}])[-1]
            if pane_now()["w"] < PANE["w"] - 100:
                print(f"JANK plan: the click narrowed the pane (a management panel opened: {sn})", flush=True); RESULTS["plan_click_opened_panel"] = {"ms": None, "ok": 0}
            print(f"INFO plan corridor {cor['id']} on {cl}: clicked {mx:.0f},{my:.0f}; vertex {to_screen(*path[1])}", flush=True)
            vx, vy = to_screen(*path[1])
            r = rt("drag_circle", vx, vy, 25, 5, 1, 120); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "plan_drag_corridor_vertex"}); time.sleep(1)
            after = json.loads(plan("get", SOCK).get("json") or "{}"); ac = next((c for c in after.get("corridors", []) if c["id"] == cor["id"]), {})
            RESULTS["plan_drag_corridor_vertex_hit"] = {"ms": None, "ok": int(ac.get("path") != cor["path"])}
            print(f"INFO plan corridor vertex moved by the drag: {ac.get('path') != cor['path']}", flush=True)
            rt("click", mx, my); time.sleep(0.4)
            for key, mods, tag in (("w", (), "plan_widen"), ("w", ("shift",), "plan_narrow"), ("a", (), "plan_approve"), ("r", (), "plan_reject")):
                b4 = json.loads(plan("get", SOCK).get("json") or "{}")
                r = rt("key", key, *mods); EVENTS.append({"kind": "switch", "t0": r["t0"], "tag": tag, "window_ms": 1500}); time.sleep(1.5)
                af = json.loads(plan("get", SOCK).get("json") or "{}")
                pick = lambda d: next(({k: c.get(k) for k in ("width", "status")} for c in d.get("corridors", []) if c["id"] == cor["id"]), {})
                print(f"INFO plan {tag}: {pick(b4)} -> {pick(af)}", flush=True)
                RESULTS[f"{tag}_applied"] = {"ms": None, "ok": int(pick(b4) != pick(af))}
            shot("plan_after_keys")
            pl = next((q for q in pj.get("planes", []) if q.get("outline") and q["outline"][0]), None)
            if pl:
                panel_click(pl["layer"], "plan_layer_plane")
                ring = pl["outline"][0]; cx_ = sum(p_[0] for p_ in ring) / len(ring); cy_ = sum(p_[1] for p_ in ring) / len(ring)
                px_, py_ = to_screen(cx_, cy_); rt("click", px_, py_); time.sleep(0.6); shot("plan_plane_selected")
                vx, vy = to_screen(*ring[0])
                r = rt("drag_circle", vx, vy, 25, 5, 1, 120); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "plan_drag_plane_vertex"}); time.sleep(1)
                after = json.loads(plan("get", SOCK).get("json") or "{}"); ap = next((q for q in after.get("planes", []) if q["id"] == pl["id"]), {})
                RESULTS["plan_drag_plane_vertex_hit"] = {"ms": None, "ok": int(ap.get("outline") != pl["outline"])}
                print(f"INFO plan plane vertex moved by the drag: {ap.get('outline') != pl['outline']}", flush=True)
        # an agent's proposal: the first corridor moved 3 mm, proposed (merge) over the API: the call to the overlay changed on screen
        panel_click(cl, "plan_layer_back")
        moved = dict(cor); moved["path"] = [[x + 3.0, y + 3.0] for x, y in cor["path"]]; moved["status"] = "proposed"; moved.pop("edited", None)
        body = {k: v for k, v in pj.items() if k in ("format", "board")}; body.update({"corridors": [moved]})
        pf = os.path.join(OUT, "propose.json"); json.dump(body, open(pf, "w"))
        t0 = now(); res = plan("propose", SOCK, pf, "merge"); t1 = now()
        EVENTS.append({"kind": "commit", "t0": t0, "tag": "plan_propose_to_overlay", "window_ms": 2000})
        perf("plan_propose_rpc", round(t1 - t0), applied=len(res.get("applied") or []), refused=len(res.get("refused") or []))
        time.sleep(2); shot("plan_proposed")
        if STATE["on"]: toggled("plan_view_off", action_toggle)  # plan view off again
# 8. schematic: pan/zoom there too
# 7c0. memory: KiCad's RSS / PSS (its process tree, smaps_rollup) after each editor a person opens: the board, the schematic, the
# footprint editor, the symbol editor, GerbView on the board's Gerbers, 3D. Each step settles 6 s. For attributing KiCad's ~4 GB.
def mem_detail(pid):
    out = {"rss_mb": None, "pss_mb": None, "anon_mb": None, "file_mb": None}
    pids = [pid]
    try:
        for p_ in os.listdir("/proc"):
            if p_.isdigit():
                try:
                    if int(open(f"/proc/{p_}/stat").read().split(")")[1].split()[1]) == pid: pids.append(int(p_))
                except Exception: pass
    except Exception: pass
    tot = {"Rss": 0, "Pss": 0, "Anonymous": 0}
    for q in pids:
        try:
            for l in open(f"/proc/{q}/smaps_rollup"):
                k = l.split(":")[0]
                if k in tot: tot[k] += int(l.split()[1])
        except Exception: pass
    out.update(rss_mb=tot["Rss"] // 1024, pss_mb=tot["Pss"] // 1024, anon_mb=tot["Anonymous"] // 1024, file_mb=(tot["Rss"] - tot["Anonymous"]) // 1024, procs=len(pids))
    return out
if "memopen" in JOURNEYS and SOCK and MEM["pid"]:
    t_ready = now()
    for at in (5, 15, 30):
        time.sleep(max(0, (t_ready + at * 1000 - now()) / 1000))
        m = mem_detail(MEM["pid"]); fr = sorted(k for k, f in kframes().items())
        RESULTS[f"memopen_{at}s"] = {"ms": None, **m, "frames": fr}
        print(f"PERF memopen_{at}s {m['rss_mb']}MB pss={m['pss_mb']} anon={m['anon_mb']} file={m['file_mb']} procs={m.get('procs')} frames={fr}", flush=True)
if "memory" in JOURNEYS and SOCK and MEM["pid"]:
    PAUSE.set()
    steps = []
    def run_procs():
        """Every process of this run (its environment names the run's folder): pid, what it is, RSS, PSS, anon, file-backed and the
        shared part (Shared_Clean + Shared_Dirty), in MB, from smaps_rollup. Each KiCad process on its own, not summed."""
        out = []
        for p_ in os.listdir("/proc"):
            if not p_.isdigit(): continue
            try:
                if OUT.encode() not in open(f"/proc/{p_}/environ", "rb").read(): continue
                cmd = open(f"/proc/{p_}/cmdline", "rb").read().split(b"\0")
                exe = os.path.basename(os.readlink(f"/proc/{p_}/exe"))
                args = " ".join(a.decode("utf-8", "replace") for a in cmd[:6])
                if not re.search(r"kicad|pcbnew|eeschema|backplane-(hub|gpui)", args + " " + exe): continue
                kind = ("api-server" if "api-server" in args else "kicad-cli" if "kicad-cli" in args else "hub" if "backplane-hub" in args
                        else "client" if "backplane-gpui" in args else "kicad-gui")
                v = {}
                for l in open(f"/proc/{p_}/smaps_rollup"):
                    k = l.split(":")[0]
                    if k in ("Rss", "Pss", "Anonymous", "Shared_Clean", "Shared_Dirty"): v[k] = int(l.split()[1]) // 1024
                out.append({"pid": int(p_), "kind": kind, "rss": v.get("Rss"), "pss": v.get("Pss"), "anon": v.get("Anonymous"),
                            "file": (v.get("Rss", 0) - v.get("Anonymous", 0)), "shared": v.get("Shared_Clean", 0) + v.get("Shared_Dirty", 0),
                            "args": args[-90:]})
            except Exception: pass
        return sorted(out, key=lambda x: (x["kind"], x["pid"]))
    def mstep(name, settle=6.0):
        time.sleep(settle); m = mem_detail(MEM["pid"]); ps = run_procs(); steps.append([name, m, ps]); RESULTS[f"mem_{name}"] = {"ms": None, **m, "procs": ps}
        print(f"PERF mem_{name} {m['rss_mb']}MB pss={m['pss_mb']} anon={m['anon_mb']} file={m['file_mb']} frames={sorted(kframes())}", flush=True)
        for q in ps:
            print(f"INFO mem_{name} {q['kind']:10s} pid={q['pid']} rss={q['rss']} pss={q['pss']} anon={q['anon']} file={q['file']} shared={q['shared']}"
                  f"{'  (the editors)' if q['pid'] == MEM['pid'] else ''}  {q['args']}", flush=True)
    mstep("board", 2.0)
    tab("SCH", "mem_sch_tab", 1.5); mstep("schematic")
    ipc("show-frame", SOCK, "fp"); mstep("footprint_editor")
    ipc("show-frame", SOCK, "sym"); mstep("symbol_editor")
    tab("PCB", "mem_back_pcb", 1.5)
    cli = os.environ.get("KICAD_CLI") or ""; mfg = os.path.join(OUT, "mfg_mem"); os.makedirs(mfg, exist_ok=True)
    if os.path.exists(cli) and PCB:
        subprocess.run([cli, "pcb", "export", "gerbers", "-o", mfg + "/", PCB], capture_output=True, timeout=300)
        subprocess.run([cli, "pcb", "export", "drill", "-o", mfg + "/", PCB], capture_output=True, timeout=300)
    gf = sorted(f for f in glob.glob(os.path.join(mfg, "*")) if re.search(r"\.(g[a-z0-9]{1,3}|drl|gbr)$", f, re.I) and not f.endswith(".gbrjob"))
    if gf: ipc("open-gerbers", SOCK, ",".join(gf), timeout=120); mstep("gerbview")
    tab("PCB_3D", "mem_3d_tab", 1.5); mstep("3d", 10.0)
    tab("PCB", "mem_back", 1.5); mstep("end", 3.0)
    # DRC in the editor, then kicad-cli DRC (a process of its own): KiCad's memory before, right after and 20 s after each
    if has_cap("embed-checks", "kiapi.backplane.embed.RunChecks"):
        since = ipc("events", SOCK, "0").get("next", 0); t0 = now(); ipc("run-checks", SOCK, "pcb")
        while now() - t0 < 120000 and not any(e["type"] == 27 for e in ipc("events", SOCK, str(since)).get("events", [])): time.sleep(0.2)
        print(f"INFO memory: in-editor DRC took {round(now() - t0)} ms", flush=True)
        mstep("after_drc_embed", 0.5); mstep("after_drc_embed_20s", 20.0)
    if os.path.exists(cli) and PCB:
        cpk = {"peak": 0}; stop_ = threading.Event()
        def cli_peak():  # kicad-cli's own RSS, its high-water mark
            while not stop_.is_set():
                for p_ in os.listdir("/proc"):
                    try:
                        if p_.isdigit() and b"kicad-cli" in open(f"/proc/{p_}/cmdline", "rb").read() and OUT.encode() in open(f"/proc/{p_}/cmdline", "rb").read():
                            for l in open(f"/proc/{p_}/status"):
                                if l.startswith("VmHWM"): cpk["peak"] = max(cpk["peak"], int(l.split()[1]) // 1024)
                    except Exception: pass
                time.sleep(0.2)
        th = threading.Thread(target=cli_peak, daemon=True); th.start()
        t0 = now(); subprocess.run([cli, "pcb", "drc", "--format", "json", "-o", os.path.join(OUT, "drc_mem.json"), PCB], capture_output=True, timeout=300)
        stop_.set(); th.join()
        print(f"INFO memory: kicad-cli DRC took {round(now() - t0)} ms, its own peak {cpk['peak']} MB", flush=True)
        RESULTS["mem_cli_drc_peak"] = {"ms": None, "mb": cpk["peak"]}
        mstep("after_drc_cli", 0.5); mstep("after_drc_cli_20s", 20.0)
    print("INFO memory steps: " + json.dumps(steps), flush=True)
    PAUSE.clear()
# 7c1. undo across a co-edit (bp.13 batch 4's fix 90eb907189): the BOM panel edits a symbol's Value (mgmt.bom.edit, as the person),
# the person's Ctrl+Z and Ctrl+Y in the schematic, then the panel edits the same symbol again: that edit must apply (it was dropped as
# "unchanged" before the fix). The Value read back from KiCad after each step; the original put back at the end.
if "undo" in JOURNEYS and SOCK:
    PAUSE.set()
    tab("SCH", "undo_sch", 1.5)
    sref = next((r_ for r_ in re.findall(r'\(property "Reference" "([RC][0-9]+)"', open(PCB, errors="ignore").read())), "")
    val = lambda: ipc("sym-value", SOCK, sref).get("value")
    orig = val() if sref else None
    steps_u = []
    def bom(value, tag):
        out = rpc([{"method": "mgmt.bom.edit", "params": {"project": KPRO, "refs": [sref], "field": "value", "value": value}}])
        time.sleep(1.0); v = val(); steps_u.append([tag, v]); print(f"INFO undo {tag}: {v!r} ({out.strip()[-140:]})", flush=True); return v
    if orig is not None:
        EXs, EYs = PANE["x"] + PANE["w"] // 4, PANE["y"] + PANE["h"] - 60   # low on the canvas's left, off the items
        v1 = bom("BP-REDO", "panel_edit")
        rt("click", EXs, EYs); time.sleep(0.4); rz = rt("key", "z", "ctrl"); time.sleep(1.2); v2 = val(); steps_u.append(["ctrl_z", v2])
        ry = rt("key", "y", "ctrl"); time.sleep(1.2); v3 = val(); steps_u.append(["ctrl_y", v3])
        print(f"INFO undo keys: ctrl_z at {rz.get('t0')}, ctrl_y at {ry.get('t0')} (epoch ms)", flush=True)
        v4 = bom("BP-AGAIN", "panel_edit_again")  # a third value: it applies only if KiCad takes the panel's second edit of the item
        print(f"INFO undo: {sref} {orig!r} -> panel {v1!r} -> Ctrl+Z {v2!r} -> Ctrl+Y {v3!r} -> panel again {v4!r}", flush=True)
        RESULTS["undo_panel_edit_applies"] = {"ms": None, "ok": int(v1 == "BP-REDO")}
        RESULTS["undo_ctrl_z_takes_it_back"] = {"ms": None, "ok": int(v2 == orig)}
        RESULTS["undo_ctrl_y_redoes_it"] = {"ms": None, "ok": int(v3 == "BP-REDO")}
        RESULTS["undo_then_panel_edit_applies"] = {"ms": None, "ok": int(v4 == "BP-AGAIN"), "steps": steps_u}
        if val() != orig: bom(orig, "put_back")
    else:
        print(f"DIAG undo: no schematic symbol {sref!r} to read", flush=True)
    tab("PCB", "undo_back", 1.2)
    PAUSE.clear()
# 7d. checks (bp.13 batch 3, embed-checks): DRC and ERC in the open editors (RunChecks) as the client's Checks panel can run them: the call
# to ET_CHECKS_DONE, a pan of the board meanwhile (does the editor stay smooth), the counts, GetMarkers, and a picture of the markers.
def wait_event(since, etype, secs, pred=lambda e: True):
    t = time.time()
    while time.time() - t < secs:
        for e in ipc("events", SOCK, str(since)).get("events", []):
            if e["type"] == etype and pred(e): return e
        time.sleep(0.1)
    return None
if "checks" in JOURNEYS and SOCK and not has_cap("embed-checks", "kiapi.backplane.embed.RunChecks"):
    print("DIAG checks: this runtime has no RunChecks (embed-checks): skipped", flush=True); RESULTS["checks_supported"] = {"ms": None, "ok": 0}
elif "checks" in JOURNEYS and SOCK:
    RESULTS["checks_supported"] = {"ms": None, "ok": 1}
    for kind, tabk, tag in (("pcb", "PCB", "drc_embed"), ("sch", "SCH", "erc_embed")):
        tab(tabk, f"{tag}_tab", 1.5)
        since = ipc("events", SOCK, "0").get("next", 0)
        box = {}
        th = None
        if kind == "pcb":
            th = threading.Thread(target=lambda: box.update(r=rt("pan", CX, CY, min(PANE["w"], PANE["h"]) // 5, 6, 2, 120))); th.start(); time.sleep(0.3)
        t0 = now(); st = ipc("run-checks", SOCK, kind); call_ms = round(now() - t0)
        done = wait_event(since, 27, 300, lambda e: not st.get("progress_id") or e["checks"].get("progress_id") == st.get("progress_id"))
        ms = round(now() - t0) if done else -1
        if th: th.join()
        if box.get("r"): EVENTS.append({"kind": "input", "t0": box["r"]["t0"], "t1": box["r"]["t1"], "tag": f"pan_during_{tag}"})
        c = (done or {}).get("checks", {}); mk = ipc("get-markers", SOCK, kind)
        perf(tag, ms, call_ms=call_ms, errors=c.get("errors"), warnings=c.get("warnings"), exclusions=c.get("exclusions"),
             unconnected=c.get("unconnected"), error=json.dumps(c.get("error") or ""), markers=mk.get("n"))
        print(f"INFO checks {tag}: GetMarkers {mk.get('counts')}", flush=True)
        time.sleep(1.0); shot(f"{tag}_markers")
    tab("PCB", "checks_back", 1.5)
# 7e. markers (bp.13 batch 3, embed-host-markers): the design review's findings drawn in the board (the hub's set "review"), as rings at
# footprints: SetHostMarkers to on screen, then a click on one (ET_MARKER_CLICKED; the hub opens its Checks panel), then cleared.
if "markers" in JOURNEYS and SOCK and not has_cap("embed-host-markers", "kiapi.backplane.embed.SetHostMarkers"):
    print("DIAG markers: this runtime has no SetHostMarkers (embed-host-markers): skipped", flush=True); RESULTS["markers_supported"] = {"ms": None, "ok": 0}
elif "markers" in JOURNEYS and SOCK:
    from pane_sampler import grab
    RESULTS["markers_supported"] = {"ms": None, "ok": 1}
    tab("PCB", "markers_board", 1.2); rt("click", CX, PANE["y"] + PANE["h"] - 30); time.sleep(0.3); rt("key", "Home"); time.sleep(1.0)
    to_scr = calibrate()
    fps = fp_positions(); refs = sorted(fps)[:: max(1, len(fps) // 8)][:8]
    ms_ = [{"id": f"rv-{r}", "title": f"review finding at {r}", "severity": i % 3, "x_mm": fps[r][0], "y_mm": fps[r][1]} for i, r in enumerate(refs)]
    CVm = (PANE["x"] + 40, PANE["y"] + 110, PANE["w"] - 80, PANE["h"] - 180)
    # where the first marker's ring is drawn: it alone, the pixels it changes (KiCad draws it a fixed size on screen)
    ring = None
    if ms_:
        ga = grab(*CVm); ipc("set-host-markers", SOCK, "pcb", "review", json.dumps(ms_[:1])); time.sleep(0.6); gb = grab(*CVm)
        c_ = changed_centroid(ga, gb, 10)
        if c_: ring = (CVm[0] + c_[0], CVm[1] + c_[1])
        ipc("set-host-markers", SOCK, "pcb", "review", "[]"); time.sleep(0.6)
        print(f"INFO markers: {ms_[0]['id']} drawn at {ring}", flush=True)
    g0 = grab(*CVm); t0 = now(); r_ = ipc("set-host-markers", SOCK, "pcb", "review", json.dumps(ms_)); call_ms = round(now() - t0)
    seen = None
    while now() - t0 < 3000:
        if float((grab(*CVm) != g0).mean()) > 0.0005: seen = round(now() - t0); break
        time.sleep(0.01)
    perf("markers_set_to_screen", seen if seen is not None else -1, call_ms=call_ms, count=r_.get("count"), sent=len(ms_))
    time.sleep(0.5); shot("markers_drawn")
    gm = ipc("get-markers", SOCK, "pcb", "host"); print(f"INFO markers: GetMarkers(include_host) {gm.get('counts')}", flush=True)
    clicked = None
    if (ring or to_scr) and ms_:
        since = ipc("events", SOCK, "0").get("next", 0); m0 = ms_[0]
        sx, sy = ring if ring else to_scr(m0["x_mm"], m0["y_mm"]); t1 = rt("click", sx, sy)["t0"]
        e = wait_event(since, 26, 3)
        clicked = e.get("marker") if e else None
        perf("marker_click_event", round(now() - t1) if e else -1, marker=json.dumps(clicked), at=f"{sx:.0f},{sy:.0f}")
        time.sleep(1.5); shot("marker_clicked")
        RESULTS["marker_click_ok"] = {"ms": None, "ok": int(bool(clicked) and clicked.get("id") == m0["id"])}
    t2 = now(); ipc("set-host-markers", SOCK, "pcb", "review", "[]"); time.sleep(0.8); shot("markers_cleared")
    RESULTS["markers_cleared"] = {"ms": None, "ok": int(ipc("get-markers", SOCK, "pcb", "host").get("counts", {}).get("host", 0) == 0)}
    rt("key", "Escape"); time.sleep(0.3)
# 7f. side panels (bp.13 batch 3, autopanels): the board in the pane as it opens, then Full size and back: what KiCad shows at each width
if "panels" in JOURNEYS and SOCK:
    tab("PCB", "panels_board", 1.2); shot("panels_narrow")
    fs = [{"id": "kicad:full"}] if PANE else []  # the pane's "Full size" (kicad_pane.rs; no rect: click_ctl clicks its measured place)
    print(f"INFO panels: Full size control {'found' if fs else 'missing'}", flush=True)
    if fs:
        t0 = click_ctl(fs[0]["id"]); time.sleep(2.0); shot("panels_wide")
        if t0: EVENTS.append({"kind": "switch", "t0": t0, "tag": "panels_full_size", "window_ms": 2000})
        t1 = click_ctl(fs[0]["id"]); time.sleep(2.0); shot("panels_back")
        if t1: EVENTS.append({"kind": "switch", "t0": t1, "tag": "panels_back", "window_ms": 2000})
        restore("panels")
    else:
        print("DIAG panels: no Full size control found", flush=True)
if "sch" in JOURNEYS:
    tab("SCH", "sch_tab", 1.5)
    rt("click", CX, PANE["y"] + PANE["h"] - 40); time.sleep(0.3)
    r = rt("pan", CX, CY, min(PANE["w"], PANE["h"]) // 5, 6, 2, 120); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "sch_pan_6s"}); time.sleep(1)
    r = rt("wheel", CX, CY, 6, 60); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "sch_zoom_6s"}); time.sleep(1)
    # cross-probe: a symbol brought to the pane's middle (SelectItems with zoom), the person's click on it, its footprint selected on the board
    SREFS = []
    sch_files = glob.glob(os.path.join(PROJ_DIR, "**", "*.kicad_sch"), recursive=True)
    for f in sch_files:
        SREFS += [r for r in re.findall(r'\(property "Reference" "([A-Z]+[0-9]+)"', open(f, errors="ignore").read()) if not r.startswith("PWR")]
    SREFS = [r for r in dict.fromkeys(SREFS) if r[0] in "RCUJDLQ"][:12:3]
    for i, ref in enumerate(SREFS[:3]):
        ipc("select-items", SOCK, "sch", ref, 1); time.sleep(0.8); rt("key", "Escape"); time.sleep(0.3)  # in view, nothing selected
        r = rt("click", CX, CY); t0 = r["t0"]; got = None
        while now() - t0 < 3000:
            sel = ipc("selection", SOCK, "pcb")
            if ref in (sel.get("refs") or []): got = now(); break
            time.sleep(0.03)
        ssel = ipc("selection", SOCK, "sch")
        perf(f"sch_xprobe{i}_{ref}", round(got - t0) if got else -1, sch_selected=int(ref in (ssel.get("refs") or [])))
    # edits: move a symbol (M, the pointer moves, a click), a wire (W, two clicks, Escape), annotate (its dialog, Return), each undone
    if SREFS:
        ipc("select-items", SOCK, "sch", SREFS[0], 1); time.sleep(0.8); rt("key", "Escape"); time.sleep(0.2)
        rt("click", CX, CY); time.sleep(0.3); rt("key", "m")
        r = rt("pan", CX + 30, CY + 30, 40, 1.2, 0, 60); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "sch_move_symbol"})
        r = rt("click", CX + 70, CY + 30); EVENTS.append({"kind": "switch", "t0": r["t0"], "tag": "sch_move_place", "window_ms": 1500}); time.sleep(1)
        rt("key", "Escape"); rt("key", "z", "ctrl"); time.sleep(1)
        rt("click", CX - 150, CY + 120); rt("key", "Escape"); rt("key", "w")
        r = rt("pan", CX - 100, CY + 120, 50, 1.0, 0, 60); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "sch_wire_draw"})
        r = rt("click", CX - 50, CY + 120); rt("key", "Escape"); rt("key", "Escape")
        EVENTS.append({"kind": "switch", "t0": r["t0"], "tag": "sch_wire_end", "window_ms": 1500}); time.sleep(1)
        rt("key", "z", "ctrl"); time.sleep(1)
    PAUSE.set()
    for act in ("eeschema.EditorControl.annotate", "eeschema.SchematicControl.annotate", "eeschema.InspectionTool.annotate"):
        if find_win("Annotate", 0.1): break
        call = subprocess.Popen(["python3", IPC, "run-action", SOCK, act], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        t0 = now(); w = find_win("Annotate", 3)
        if w:
            perf("annotate_dialog_open", round(now() - t0), action=act); sh("python3", XIN, "focus", w["title"]); time.sleep(0.2)
            t1 = rt("key", "Return").get("t0"); worst, busy = busy_after(t1, 6); perf("annotate_run_busy", busy, ping_max=worst)
            if find_win("Annotate", 0.2): sh("python3", XIN, "focus", "Annotate"); rt("key", "Escape"); time.sleep(0.4)
            try: call.wait(timeout=5)
            except Exception: call.kill()
            rt("key", "z", "ctrl"); time.sleep(0.5)
            break
        try: call.kill()
        except Exception: pass
    else: print("DIAG annotate: no action opened its dialog", flush=True)
    PAUSE.clear()
    # a hierarchical sheet: not yet (eeschema.NavigateTool.enterSheet needs a sheet selected; SelectItems takes references only)
    tab("PCB", "sch_back_board", 1.5)

# 6. comment: the fork's "Comment to Backplane" on a selection -> the message in the thread
if "comment" in JOURNEYS and SOCK:
    tab("PCB", "comment_board", 1.2); rt("click", CX, CY + PANE["h"] // 3); time.sleep(0.5)
    for i in range(3):
        if not comment_window(f"comment_window{i}"):
            # after the window was closed with Escape, does Ctrl+; need another click into KiCad? (the focus bug's other shape)
            print("JANK Ctrl+; did not open the comment window after the last one was closed with Escape; a click into KiCad and again:", flush=True)
            rt("click", CX, CY + PANE["h"] // 3); time.sleep(0.6)
            RESULTS[f"comment_window{i}_after_click"] = {"ms": None, "ok": int(comment_window(f"comment_window{i}_retry"))}
            RESULTS["reserved_after_escape"] = {"ms": None, "ok": 0}
    ev = os.path.join(OUT, "home", "events.jsonl"); n0 = sum(1 for _ in open(ev, errors="ignore"))
    t0 = now(); r = ipc("comment-selection", SOCK, "pcb", "real-use comment probe"); t_ipc = now()
    posted = None
    while now() - t0 < 10000 and posted is None:
        lines = open(ev, errors="ignore").read().splitlines()[n0:]
        if any("real-use comment probe" in l for l in lines): posted = now()
        else: time.sleep(0.02)
    perf("comment_ipc", round(t_ipc - t0), ok=int(bool(r.get("ok"))))
    perf("comment_posted", round(posted - t0) if posted else -1)
shot("end_journeys")
STOP.set()
path = sampler_stop()
res = PRE + slow + analyze(EVENTS, path)
for r in res:
    tag = r.pop("tag"); kind = r.pop("kind")
    if kind == "input": perf(tag, r.get("p99"), **{k: v for k, v in r.items() if k != "p99"})
    elif kind == "commit": perf(tag, r.get("first_change_ms"))
    else: perf(tag, r.get("settled_ms"), **{k: r.get(k) for k in ("first_change_ms", "mid_frames", "blank_frames", "changes")})
if MEM["samples"]:
    RESULTS["rss_peak_mb"] = {"ms": None, "mb": max(x[1] for x in MEM["samples"]), "hwm_mb": max(x[2] for x in MEM["samples"])}
    RESULTS["rss_end_mb"] = {"ms": None, "mb": MEM["samples"][-1][1]}
    print(f"PERF memory ready={RESULTS['rss_ready_mb']['mb']}MB peak={RESULTS['rss_peak_mb']['mb']}MB hwm={RESULTS['rss_peak_mb']['hwm_mb']}MB end={RESULTS['rss_end_mb']['mb']}MB", flush=True)
RESULTS["switch_shows_the_asked_frame"] = {"ms": None, "ok": int(not SHOW_MISSES), "misses": SHOW_MISSES}
summary = {"results": RESULTS, "client": os.environ.get("CLIENT_BIN"), "kicad": os.environ.get("KICAD_BIN"), "kicad_running": KICAD_RUN, "dialogs": DIALOGS, "pane": PANE, "gpu": (metrics("gpu") or [{}])[0], "cold_edit_click_epoch": edit_click.get("epoch_ms")}
json.dump(summary, open(os.path.join(OUT, "real_use.json"), "w"), indent=1)
print("JSON " + json.dumps(summary), flush=True)
