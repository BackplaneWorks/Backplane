#!/usr/bin/env python3
"""What the screen shows in a rectangle, as fast as the X server hands it over (XGetImage of the root: what the person sees, composited).
  pane_sampler.py run X Y W H OUT.jsonl STOPFILE [MAX_S]   one line per sample: {"t": epoch ms, "h": crc32, "u": share of the mode colour, "c": mode}
  pane_sampler.py analyze OUT.jsonl EVENTS.json            EVENTS: [{"kind":"switch"|"input"|"commit","t0":epoch ms,"t1":epoch ms (input end),"tag":...}]
Analysis per event, from the samples:
  switch: first_change_ms (old content shown until), settled_ms (the last change before 400 ms of stillness: click -> drawn),
          blank_frames (distinct pictures in between that are one colour: a flash of background), mid_frames (distinct pictures between old and final)
  input:  while input runs, the times between changed pictures (a frame-time distribution: p50/p95/p99/max, hitches > 33 and > 50 ms)
  commit: first_change_ms (an agent's commit to the first repaint)
Sampling costs about 3 to 6 ms a picture for an 800x900 pane, so intervals below that are not seen; a frame shown for less than one sample is missed."""
import ctypes, ctypes.util, json, os, sys, time, zlib

def run(x, y, w, h, out, stop, max_s):
    import numpy as np
    x11 = ctypes.CDLL(ctypes.util.find_library("X11"))
    x11.XOpenDisplay.restype = ctypes.c_void_p; x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XDefaultRootWindow.restype = ctypes.c_ulong; x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    x11.XGetImage.restype = ctypes.c_void_p
    x11.XGetImage.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint, ctypes.c_ulong, ctypes.c_int]
    x11.XDestroyImage.argtypes = [ctypes.c_void_p]
    # XImage: width, height, xoffset, format, data (char*) ... bytes_per_line at a fixed offset; read data pointer at offset 16 and bpl at 44 (x86_64)
    d = x11.XOpenDisplay(os.environ.get("DISPLAY", ":0").encode())
    if not d: sys.exit("cannot open display")
    root = x11.XDefaultRootWindow(d)
    f = open(out, "w", buffering=1)
    t_end = time.time() + max_s; last = None; last_t = 0.0
    band = int(os.environ.get("KEMBED_SAMPLE_BANDS", "0") or 0)
    gap = float(os.environ.get("KEMBED_SAMPLE_MIN_MS", "0")) / 1000  # a floor between samples: how much the sampling itself costs the frames
    while time.time() < t_end and not os.path.exists(stop):
        t = time.time()
        if gap and last is not None and t - last_t < gap: time.sleep(gap - (t - last_t)); t = time.time()
        last_t = t
        img = x11.XGetImage(d, root, x, y, w, h, 0xFFFFFFFF, 2)
        if not img: time.sleep(0.01); continue
        data = ctypes.c_void_p.from_address(img + 16).value
        bpl = ctypes.c_int.from_address(img + 44).value
        buf = ctypes.string_at(data, bpl * h)
        x11.XDestroyImage(img)
        hv = zlib.crc32(buf)
        if hv == last:  # unchanged: one short line keeps the timeline (a still picture is a sample too)
            f.write(f'{{"t":{t*1000:.1f},"h":{hv}}}\n'); continue
        a = np.frombuffer(buf, dtype=np.uint32).reshape(h, bpl // 4)[::7, :w:7] & 0xFFFFFF
        vals, counts = np.unique(a, return_counts=True); i = int(counts.argmax())
        row = {"t": round(t * 1000, 1), "h": hv, "u": round(float(counts[i]) / a.size, 4), "c": int(vals[i])}
        # KEMBED_SAMPLE_BANDS=N: a crc per N-px-wide column band too, so a change can be placed (KiCad's canvas or a panel beside it)
        if band:
            full = np.frombuffer(buf, dtype=np.uint32).reshape(h, bpl // 4)[:, :w]
            row["b"] = [zlib.crc32(full[:, i:i + band].tobytes()) for i in range(0, w, band)]
        f.write(json.dumps(row) + "\n")
        last = hv
    f.close()

_GRAB = {}
def grab(x, y, w, h):
    """One picture of a root rectangle as a numpy array (h, w) of 0xRRGGBB, for the drivers' pixel diffs."""
    import numpy as np
    if not _GRAB:
        x11 = ctypes.CDLL(ctypes.util.find_library("X11"))
        x11.XOpenDisplay.restype = ctypes.c_void_p; x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
        x11.XDefaultRootWindow.restype = ctypes.c_ulong; x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
        x11.XGetImage.restype = ctypes.c_void_p
        x11.XGetImage.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint, ctypes.c_ulong, ctypes.c_int]
        x11.XDestroyImage.argtypes = [ctypes.c_void_p]
        d = x11.XOpenDisplay(os.environ.get("DISPLAY", ":0").encode())
        _GRAB.update(x11=x11, d=d, root=x11.XDefaultRootWindow(d))
    x11, d = _GRAB["x11"], _GRAB["d"]
    img = x11.XGetImage(d, _GRAB["root"], x, y, w, h, 0xFFFFFFFF, 2)
    data = ctypes.c_void_p.from_address(img + 16).value; bpl = ctypes.c_int.from_address(img + 44).value
    a = np.frombuffer(ctypes.string_at(data, bpl * h), dtype=np.uint32).reshape(h, bpl // 4)[:, :w] & 0xFFFFFF
    x11.XDestroyImage(img)
    return a.copy()

def pct(v, q):
    if not v: return None
    v = sorted(v); return v[min(len(v) - 1, max(0, int(-(-q * len(v) // 1)) - 1))]

def analyze(path, events_path):
    S = [json.loads(l) for l in open(path) if l.strip()]
    meta = {}
    for s in S:  # carry u/c forward to repeated samples
        if "u" in s: meta[s["h"]] = (s["u"], s["c"])
        s["u"], s["c"] = meta.get(s["h"], (0, 0))
    out = []
    for ev in json.load(open(events_path)):
        t0 = ev["t0"]; kind = ev["kind"]; r = {"tag": ev.get("tag"), "kind": kind}
        before = [s for s in S if s["t"] < t0]; after = [s for s in S if s["t"] >= t0 and s["t"] <= t0 + ev.get("window_ms", 5000)]
        if not after: r["error"] = "no samples"; out.append(r); continue
        h_old = before[-1]["h"] if before else None
        if kind in ("switch", "commit"):
            ch = next((s for s in after if s["h"] != h_old), None)
            r["first_change_ms"] = round(ch["t"] - t0, 1) if ch else None
            if kind == "switch":
                # settled: the last change followed by >= 400 ms with no change (or the window's end)
                changes = [s for i, s in enumerate(after) if s["h"] != (after[i - 1]["h"] if i else h_old)]
                settled = None
                for i, c in enumerate(changes):
                    nxt = changes[i + 1]["t"] if i + 1 < len(changes) else after[-1]["t"] + 1e9
                    if nxt - c["t"] >= 400: settled = c; break
                final = settled["h"] if settled else after[-1]["h"]
                r["settled_ms"] = round(settled["t"] - t0, 1) if settled else None
                mids = []
                for s in after:
                    if settled and s["t"] >= settled["t"]: break
                    if s["h"] not in (h_old, final) and s["h"] not in [m["h"] for m in mids]: mids.append(s)
                r["mid_frames"] = len(mids)
                r["blank_frames"] = sum(1 for m in mids if m["u"] >= 0.97)
                r["changes"] = len(changes)
        elif kind == "input":
            t1 = ev.get("t1", t0 + 1000)
            span = [s for s in S if t0 <= s["t"] <= t1 + 100]
            ct = [s["t"] for i, s in enumerate(span) if i and s["h"] != span[i - 1]["h"]]
            iv = [round(b - a, 1) for a, b in zip(ct, ct[1:])]
            r.update({"frames": len(ct), "first_change_ms": round(ct[0] - t0, 1) if ct else None,
                      "p50": pct(iv, .5), "p95": pct(iv, .95), "p99": pct(iv, .99), "max": max(iv) if iv else None,
                      "hitch33": sum(1 for x in iv if x > 33), "hitch50": sum(1 for x in iv if x > 50),
                      "samples": len(span), "sample_ms": round((span[-1]["t"] - span[0]["t"]) / max(1, len(span) - 1), 2) if span else None})
        out.append(r)
    print(json.dumps(out))

if __name__ == "__main__":
    if sys.argv[1] == "run":
        x, y, w, h = map(int, sys.argv[2:6]); run(x, y, w, h, sys.argv[6], sys.argv[7], float(sys.argv[8]) if len(sys.argv) > 8 else 600)
    elif sys.argv[1] == "analyze": analyze(sys.argv[2], sys.argv[3])
