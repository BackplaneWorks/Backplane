"""embed_marker_click_e2e's drive: a person's click on one of KiCad's DRC markers in the embedded editor, then Escape, Delete, the chip and
Escape again. Prints `CHECK <0|1> <what>|<detail>` lines for the suite.
argv: OUT, the client's `ready` kicad report (JSON), KiCad's socket, the board file, gui_ipc_probe.py, xinput.py."""
import json, os, re, subprocess, sys, time

OUT, R, SOCK, PCB, IPC, XIN = sys.argv[1], json.loads(sys.argv[2] or "{}"), sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6]
CLOG, HLOG = os.path.join(OUT, "client.log"), os.path.join(OUT, "hub.log")


def check(ok, what, detail=""): print(f"CHECK {int(bool(ok))} {what}|{detail}", flush=True)
def info(s): print(f"INFO {s}", flush=True)
def lines(p): return open(p, errors="replace").read().splitlines()
def x(*a): subprocess.run(["python3", XIN, *map(str, a)], check=False)
def ipc(*a):
    p = subprocess.run(["python3", IPC, a[0], SOCK, *map(str, a[1:])], capture_output=True, text=True)
    try: return json.loads(p.stdout.strip().splitlines()[-1])
    except Exception: return {"error": (p.stdout + p.stderr)[-300:]}


def geom(xid):
    """A window's place on the screen now: (x, y, w, h, viewable)."""
    o = subprocess.run(["xwininfo", "-id", str(xid)], capture_output=True, text=True).stdout
    g = lambda k: int(re.search(rf"{k}:\s+(-?\d+)", o).group(1)) if re.search(rf"{k}:\s+(-?\d+)", o) else 0
    return (g("Absolute upper-left X"), g("Absolute upper-left Y"), g("Width"), g("Height"), "IsViewable" in o)


def metrics(since, pred):
    out = []
    for l in lines(CLOG)[since:]:
        for tag in ("GPUI_METRIC ", "GPUI_LOG "):
            if tag in l:
                try: o = json.loads(l.split(tag, 1)[1])
                except Exception: continue
                if pred(o): out.append(o)
    return out


def closed_since(n): return [m for m in metrics(n, lambda o: o.get("event") == "shown") if not m.get("open")]


def rect_now(cid):
    """The control's latest drawn rect (BACKPLANE_GPUI_RECTS' probes), in the client window's coordinates."""
    for m in reversed(metrics(0, lambda o: isinstance(o.get("rects"), list))):
        r = next((r for r in m["rects"] if r.get("id") == cid), None)
        if r: return r
    return None


shown = R.get("shown"); top = R.get("toplevel")
fr = next((f for f in R.get("frames", []) if f.get("xid") == shown), {})
if not shown or not fr:
    check(0, "the board editor is shown", json.dumps(R)[:300]); sys.exit()
FX, FY, FW, FH, _ = geom(shown)
TX, TY = geom(top)[:2] if top else (0, 0)
pane = R.get("pane") or {}
PX, PY, PW, PH = TX + int(pane.get("x", 0)), TY + int(pane.get("y", 0)), int(pane.get("w", FW)), int(pane.get("h", FH))
info(f"KiCad's frame {FX},{FY} {FW}x{FH}; pane {PX},{PY} {PW}x{PH}; window at {TX},{TY}")

# zoom to fit (real_use's Home): a click into the canvas low down, then Home
x("click", FX + FW // 2, FY + FH - 120); time.sleep(0.6); x("key", "Home"); time.sleep(1.5)


def candidates():
    """Points to try: the pane's centre (Hubble's zoomed-to-fit centre is a marker: real_use's select click), then the tips of KiCad's
    DRC-error pink nearest it (a marker's arrow is drawn from its tip down and to the right)."""
    pts = [(PX + PW // 2, PY + PH // 2)]
    shot = os.path.join(OUT, "shots", "before_marker.png")
    subprocess.run(["import", "-window", "root", shot], check=False)
    try:
        import numpy as np
        from PIL import Image
        a = np.asarray(Image.open(shot).convert("RGB")).astype(int)[FY:FY + FH, FX:FX + FW]
        r, g, b = a[..., 0], a[..., 1], a[..., 2]
        m = (r > 140) & (r - g > 60) & (b > g) & (r - b > 40) & (b < 170)
        ys, xs = np.nonzero(m)
        cells = {}
        for X_, Y_ in zip(xs.tolist(), ys.tolist()):
            k = (X_ // 60, Y_ // 60)
            if k not in cells or X_ + Y_ < sum(cells[k]): cells[k] = (X_, Y_)
        cx, cy = FW // 2, FH // 2
        tips = sorted(cells.values(), key=lambda p: (p[0] - cx) ** 2 + (p[1] - cy) ** 2)
        pts += [(FX + p[0] + 6, FY + p[1] + 6) for p in tips[:5]]
    except Exception as e:
        info(f"no picture to look for markers in: {e}")
    return pts


hit = None
for (cx, cy) in candidates():
    n_hub, n_cli = len(lines(HLOG)), len(lines(CLOG))
    g0 = geom(shown)
    x("click", cx, cy); time.sleep(2.0)
    rows = [re.search(r"marker clicked: row (\S+)", l) for l in lines(HLOG)[n_hub:]]
    rows = [m.group(1) for m in rows if m]
    if rows:
        hit = (cx, cy, rows[-1], g0, n_cli); break
    info(f"no marker at {cx},{cy}")
if not hit:
    check(0, "a click landed on one of KiCad's DRC markers (the hub named its row)", "none of the points tried; shots/before_marker.png"); sys.exit()
cx, cy, row, g0, n_cli = hit
info(f"marker clicked at {cx},{cy}: row {row}")
subprocess.run(["import", "-window", "root", os.path.join(OUT, "shots", "marker_clicked.png")], check=False)

# M1: the editor where it was, the panel shut
g1 = geom(shown)
check(g1[:4] == g0[:4], "M1 a marker click leaves KiCad's frame where it was (no panel opened beside it, the canvas not moved)", f"before {g0[:4]} after {g1[:4]}")
panel = [m for m in metrics(n_cli, lambda o: isinstance(o.get("rects"), list)) if any(str(r.get("id", "")).startswith("mgmt:checks:") for r in m["rects"])]
check(not panel, "M1 the Checks panel stays shut", f"{len(panel)} frames drew it")
# M2: the chip says it
fl = metrics(n_cli, lambda o: o.get("event") == "checks_chip_flash")
check(bool(fl), "M2 the strip's checks chip says the clicked finding", json.dumps(fl[:1]))

# M3: Escape twice keeps the editor; Delete reaches KiCad
n3 = len(lines(CLOG))
x("key", "Escape"); time.sleep(0.4); x("key", "Escape"); time.sleep(1.2)
check(not closed_since(n3) and geom(shown)[4], "M3 Escape twice after the marker click keeps the editor shown", f"closed: {closed_since(n3)[:1]}")
ref = next(iter(re.findall(r'\(property "Reference" "([A-Z]+[0-9]+)"', open(PCB, errors="replace").read())), "")
m0 = ipc("modified", "pcb")
sel = ipc("select-items", "pcb", ref, 0); time.sleep(0.5)
x("key", "Delete"); time.sleep(1.5)
m1 = ipc("modified", "pcb")
check(m0.get("modified") is False and m1.get("modified") is True, f"M3 Delete then reaches KiCad ({ref} selected and deleted: the board becomes modified)", f"before {m0} after {m1}; select {json.dumps(sel)[:160]}")

# M4: the chip opens Checks on the row
chip = rect_now("kicad:checks-chip")
n4 = len(lines(CLOG))
if chip:
    x("click", TX + int(chip["x"] + chip["w"] / 2), TY + int(chip["y"] + chip["h"] / 2)); time.sleep(2.5)
rev = [m.get("id") for m in metrics(n4, lambda o: o.get("event") == "checks_reveal")]
check(row in rev, "M4 a press on the chip opens Checks on the clicked row", f"chip {chip}, revealed {rev}")

# M5: Escape in Backplane: the panel first, then the viewer; KiCad keeps the unsaved edit
n5 = len(lines(CLOG))
x("key", "Escape"); time.sleep(1.5)
shut = metrics(n5, lambda o: o.get("mgmt_toggle") == "checks" and o.get("open") == "checks")
check(bool(shut) and not closed_since(n5), "M5 Escape in Backplane shuts the open Checks panel, the viewer stays", f"toggles {shut[:1]}, closed {closed_since(n5)[:1]}")
x("key", "Escape"); time.sleep(1.5)
check(bool(closed_since(n5)), "M5 a second Escape closes the viewer", "")
m2 = ipc("modified", "pcb")
check(m2.get("modified") is True, "M5 the viewer closed, KiCad keeps the unsaved edit (its session not closed under it)", json.dumps(m2))
