#!/usr/bin/env python3
"""The real-use journeys, driven from outside the client as a person would: XTest clicks on the client's controls (rects from its
`controls` metric), XTest pan/zoom/keys into KiCad, an agent's commits over KiCad's API, the hub's RPCs; the pane's pixels sampled
throughout (pane_sampler.py). Writes $OUT/real_use.json and prints PERF lines.
  real_use_drive.py OUT CLIENT_LOG JOURNEYS     JOURNEYS: comma list of open,switch,edit,coedit,xprobe,comment,save,sch (default all)
Environment from env_xvfb.sh: DISPLAY, HPORT, IPC_PY, DRIVER_PY, HUB_RPC_MJS.
A KiCad dialog that appears on its own (a load warning) is recorded with its title and time and dismissed with Return: it blocks KiCad."""
import json, os, re, subprocess, sys, threading, time

OUT, CLOG = sys.argv[1], sys.argv[2]
JOURNEYS = (sys.argv[3] if len(sys.argv) > 3 else "focus,open,switch,edit,mgmt,coedit,xprobe,comment,save,tools,plan,sch").split(",")
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
    if not r and cid.startswith("kicad-tab:") and PANE:
        print(f"DIAG {cid}: no rect, clicking its measured place", flush=True) if cid not in WARNED else None; WARNED.add(cid)
        return rt("click", PANE["x"] + TAB_AT[cid.split(":", 1)[1]], PANE["y"] - 15).get("t0")
    if not r: print(f"DIAG no control {cid}", flush=True); return None
    return rt("click", GX + r["x"] + r["w"] / 2, GY + r["y"] + r["h"] / 2).get("t0")
def kicad_report(tag):
    rs = [m for m in metrics("kicad") if m.get("tag") == tag]
    return rs[-1] if rs else {}
def windows(): return json.loads(sh("python3", DRV, "list-windows") or "[]")

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
def tab(kind, tag, settle=1.4):
    t0 = click_ctl(f"kicad-tab:{kind}")
    if t0: EVENTS.append({"kind": "switch", "t0": t0, "tag": tag, "window_ms": settle * 1000}); time.sleep(settle)

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
if "focus" in JOURNEYS:
    rt("click", CX, CY + PANE["h"] // 3); time.sleep(0.6)
    # Ctrl+K (Backplane's command palette, a reserved key) right after the one click: the palette's place over the chat changes?
    box = {"x": 600, "y": 0, "w": 700, "h": 420}
    sampler_stop(); sampler_start(box, "ctrlk"); r = rt("key", "k", "ctrl"); time.sleep(1.2); kp = sampler_stop(); sampler_start(PANE, "pane")
    a = analyze([{"kind": "commit", "t0": r["t0"], "tag": "ctrlk", "window_ms": 1000}], kp)
    ck = a[0].get("first_change_ms") if a else None
    perf("first_click_ctrl_k", ck if ck is not None else -1, opened=int(ck is not None)); RESULTS["first_click_ctrl_k_ok"] = {"ms": None, "ok": int(ck is not None)}
    rt("key", "Escape"); time.sleep(0.4); rt("click", CX, CY + PANE["h"] // 3); time.sleep(0.6)
    ok1 = comment_window("first_click_ctrl_semicolon")
    RESULTS["first_click_reserved"] = {"ms": None, "ok": int(ok1)}
    if not ok1:
        print("JANK first click into KiCad: Ctrl+; did not reach Backplane; a second click:", flush=True)
        rt("click", CX, CY + PANE["h"] // 3); time.sleep(0.6)
        RESULTS["second_click_reserved"] = {"ms": None, "ok": int(comment_window("second_click_ctrl_semicolon"))}
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
        EVENTS.append({"kind": "switch", "t0": t0, "tag": f"xprobe{i}_{ref}", "window_ms": 2000})
        perf(f"xprobe_rpc_{i}", round(t1 - t0), ref=ref, ok=int('"ok":true' in out.replace(" ", "")))
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
def fp_positions():
    txt = open(PCB, errors="ignore").read(); out = {}
    for m in re.finditer(r'\(footprint "[^"]*"(.*?)\n\t\)', txt, re.S):
        body = m.group(1); ref = re.search(r'\(property "Reference" "([^"]+)"', body)
        at = re.search(r'\n\t\t\(at ([-\d.]+) ([-\d.]+)', body) or re.search(r'\n\t\t\(transform\s*\(translate ([-\d.]+) ([-\d.]+)\)', body)  # 10.0 / 10.99 forms
        if at and ref: out[ref.group(1)] = (float(at.group(1)), float(at.group(2)))
    return out
def calibrate():
    from pane_sampler import grab
    # small two-pad parts (C, R): their origin is their middle, which is where a selection's changed pixels centre
    fps = fp_positions(); refs = [r for r in fps if re.match(r"^[CR]\d+$", r)] or [r for r in fps if re.match(r"^[A-Z]+\d+$", r)]
    if len(refs) < 2: print(f"DIAG plan calibration: {len(refs)} footprints parsed", flush=True); return None
    # two far-apart footprints
    a = min(refs, key=lambda r: fps[r][0] + fps[r][1]); b = max(refs, key=lambda r: fps[r][0] + fps[r][1])
    pts = []
    for ref in (a, b):
        # the canvas only (not the toolbars, the header or the status bar, which a selection changes too): measured at 1800x1050
        CV = (PANE["x"] + 40, PANE["y"] + 110, 720, 715)
        rt("key", "Escape"); time.sleep(0.4); g0 = grab(*CV)
        ipc("select-items", SOCK, "pcb", ref, 0); time.sleep(0.6); g1 = grab(*CV)
        c = changed_centroid(g0, g1)
        if c: c = (c[0] + 40, c[1] + 110)
        if not c: print(f"DIAG plan calibration: selecting {ref} changed no pixels", flush=True); return None
        pts.append((fps[ref], (PANE["x"] + c[0], PANE["y"] + c[1])))
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
        tab("PCB", "plan_board", 1.2); rt("click", CX, PANE["y"] + PANE["h"] - 30); time.sleep(0.3); rt("key", "Home"); time.sleep(1.0)
        to_screen = calibrate()
        RESULTS["plan_calibrated"] = {"ms": None, "ok": int(to_screen is not None)}
        r = rt("key", "p", "alt"); time.sleep(0.6)
        menu = [w for w in windows() if w.get("shown") and w.get("override_redirect") and w["w"] > 100 and w["h"] > 100]
        RESULTS["plan_alt_p_is_plan_view"] = {"ms": None, "ok": int(not menu)}
        if menu:
            print(f"JANK Alt+P opened a menu ({menu[0]['w']}x{menu[0]['h']} popup: KiCad's Place menu mnemonic) instead of the plan view; driving plan view by its action", flush=True)
            shot("plan_alt_p_menu"); rt("key", "Escape"); time.sleep(0.3); rt("key", "Escape"); time.sleep(0.5)
        def toggle_plan():
            t0 = now(); st = ipc("run-action", SOCK, "pcbnew.Plan.togglePlanView")
            if not st.get("ok"): print(f"DIAG togglePlanView: {st}", flush=True)
            return t0
        EVENTS.append({"kind": "switch", "t0": r["t0"], "tag": "plan_alt_p", "window_ms": 1500})
        if menu: t0 = toggle_plan()  # Alt+P opened a menu: the plan view by its action (RunAction may not reach it: its status is printed)
        else: t0 = r["t0"]
        EVENTS.append({"kind": "switch", "t0": t0, "tag": "plan_view_on", "window_ms": 2000}); time.sleep(2); shot("plan_view")
        for i in range(10):
            t0 = toggle_plan() if menu else rt("key", "p", "alt")["t0"]
            EVENTS.append({"kind": "switch", "t0": t0, "tag": f"plan_toggle{i:02d}_{'off' if i % 2 == 0 else 'on'}", "window_ms": 1200}); time.sleep(1.2)
        cor = pj["corridors"][0]; cl = cor["layer"]
        order = sorted({l["layer"] for l in pj.get("layers", [])}, key=lambda n: 0 if n == "F.Cu" else (99 if n == "B.Cu" else int(re.sub(r"\D", "", n) or 50)))
        def panel_click(layer, tag):  # the plan view's mini panel for LAYER (measured at 1800x1050: x pane+708, y pane+179+120*i)
            i = order.index(layer) if layer in order else 0
            r = rt("click", PANE["x"] + 708, PANE["y"] + 179 + 120 * i)
            EVENTS.append({"kind": "switch", "t0": r["t0"], "tag": tag, "window_ms": 1500}); time.sleep(1.5)
            print(f"INFO plan layer after the {layer} panel: {plan('layer', SOCK).get('layer')}", flush=True)
        panel_click(cl, "plan_layer_corridor"); shot("plan_layer_corridor")
        if to_screen:
            path = cor["path"]; mid = ((path[0][0] + path[1][0]) / 2, (path[0][1] + path[1][1]) / 2)
            mx, my = to_screen(*mid); rt("click", mx, my); time.sleep(0.6); shot("plan_corridor_selected")
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
        (toggle_plan() if menu else rt("key", "p", "alt")); time.sleep(1)  # plan view off again
# 8. schematic: pan/zoom there too
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
summary = {"results": RESULTS, "client": os.environ.get("CLIENT_BIN"), "kicad": os.environ.get("KICAD_BIN"), "kicad_running": KICAD_RUN, "dialogs": DIALOGS, "pane": PANE, "gpu": (metrics("gpu") or [{}])[0], "cold_edit_click_epoch": edit_click.get("epoch_ms")}
json.dump(summary, open(os.path.join(OUT, "real_use.json"), "w"), indent=1)
print("JSON " + json.dumps(summary), flush=True)
