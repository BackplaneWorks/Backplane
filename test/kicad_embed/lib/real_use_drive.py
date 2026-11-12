#!/usr/bin/env python3
"""The real-use journeys, driven from outside the client as a person would: XTest clicks on the client's controls (rects from its
`controls` metric), XTest pan/zoom/keys into KiCad, an agent's commits over KiCad's API, the hub's RPCs; the pane's pixels sampled
throughout (pane_sampler.py). Writes $OUT/real_use.json and prints PERF lines.
  real_use_drive.py OUT CLIENT_LOG JOURNEYS     JOURNEYS: comma list of open,switch,edit,coedit,xprobe,comment,save,sch (default all)
Environment from env_xvfb.sh: DISPLAY, HPORT, IPC_PY, DRIVER_PY, HUB_RPC_MJS.
A KiCad dialog that appears on its own (a load warning) is recorded with its title and time and dismissed with Return: it blocks KiCad."""
import json, os, re, subprocess, sys, threading, time

OUT, CLOG = sys.argv[1], sys.argv[2]
JOURNEYS = (sys.argv[3] if len(sys.argv) > 3 else "focus,open,switch,edit,mgmt,coedit,xprobe,comment,save,sch").split(",")
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

# --- dialogs: KiCad windows that are neither an editor frame nor embedded: recorded, then Return
STOP = threading.Event(); KNOWN = set()
def dialog_watch():
    while not STOP.is_set():
        try:
            for w in windows():
                if not w.get("shown") or w.get("override_redirect") or w["w"] < 120 or w["h"] < 60: continue
                if "kicad" not in w.get("wm_class", "").lower(): continue
                t = w.get("title", "")
                if re.search(r"(PCB Editor|Schematic Editor|3D Viewer|Footprint Editor|Symbol Editor)$", t) or w["xid"] in KNOWN: continue
                if not w.get("transient_for") and t not in ("Load Schematic", "Load PCB", "Error", "Warning", "Confirmation", "Information"): continue
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
cold = wait_metric(lambda m: m.get("tag") == "cold", 180)
if not cold: print("FAIL no cold report"); sys.exit(1)
edit_click = next((m for m in metrics("click") if m.get("id") == "ui:Edit"), {})
perf("open_cold_painted", cold.get("wait_ms"), state=cold.get("state"))
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
time.sleep(1.2); RESULTS["rss_ready_mb"] = {"ms": None, "mb": mem_now()}
CONT = int((geo.get("container") or {}).get("xid") or 0)
print(f"INFO pane={PANE} sock={SOCK} kicad_pid={MEM['pid']} rss={RESULTS['rss_ready_mb']['mb']}MB gpu={(metrics('gpu') or [{}])[0].get('device')} client={os.environ.get('CLIENT_BIN')}", flush=True)
CX, CY = PANE["x"] + PANE["w"] // 2, PANE["y"] + PANE["h"] // 2
sampler_start(PANE, "pane")

def shot(name): sh("import", "-window", "root", os.path.join(OUT, "shots", f"{name}.png"), timeout=20)
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
    # threads: away through the command palette (Jump or run a command) and back the same way, as a person with the keyboard does
    def jump(title):
        click_ctl("ui:Jump or run a command"); time.sleep(0.4)
        sh("python3", os.path.join(os.environ["REPO_ROOT"], "test/cef/xinput.py"), "type", title); time.sleep(0.5)
        return rt("key", "Return").get("t0")
    for i in range(3):
        jump("Other thread 2"); time.sleep(1.5)
        t0 = jump("Embedded KiCad E2E")
        if t0: EVENTS.append({"kind": "switch", "t0": t0, "tag": f"thread_back{i}", "window_ms": 3000}); time.sleep(3)
        shot(f"thread_back{i}")
    restore("thread_switch")
    # the same through the sidebar: the project's row lists its threads; a click on the other one, then on this one
    def side(tid):  # the sidebar's row when it lists the thread (Active), else the project's row and its page's thread row
        if control(f"thread:{tid}"): return click_ctl(f"thread:{tid}")
        click_ctl("project:p1"); time.sleep(1.0)
        return click_ctl(f"page-thread:{tid}")
    for i in range(3):
        if not side("t2"): print("DIAG sidebar: no thread:t2 or page-thread:t2 control", flush=True); break
        time.sleep(1.5); t0 = side("t1")
        if t0: EVENTS.append({"kind": "switch", "t0": t0, "tag": f"thread_side_back{i}", "window_ms": 3000}); time.sleep(3)
        shot(f"thread_side_back{i}")
    restore("thread_sidebar")
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
# 8. schematic: pan/zoom there too
if "sch" in JOURNEYS:
    tab("SCH", "sch_tab", 1.5)
    rt("click", CX, PANE["y"] + PANE["h"] - 40); time.sleep(0.3)
    r = rt("pan", CX, CY, min(PANE["w"], PANE["h"]) // 5, 6, 2, 120); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "sch_pan_6s"}); time.sleep(1)
    r = rt("wheel", CX, CY, 6, 60); EVENTS.append({"kind": "input", "t0": r["t0"], "t1": r["t1"], "tag": "sch_zoom_6s"}); time.sleep(1)
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
summary = {"results": RESULTS, "client": os.environ.get("CLIENT_BIN"), "kicad": os.environ.get("KICAD_BIN"), "dialogs": DIALOGS, "pane": PANE, "gpu": (metrics("gpu") or [{}])[0], "cold_edit_click_epoch": edit_click.get("epoch_ms")}
json.dump(summary, open(os.path.join(OUT, "real_use.json"), "w"), indent=1)
print("JSON " + json.dumps(summary), flush=True)
