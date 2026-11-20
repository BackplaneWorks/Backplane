#!/usr/bin/env python3
"""Sample a KiCad's quit as it happens, every ~150 ms, and print each change with its time since the start (ms):
  sock    KiCad's API socket: answers a Ping (yes), connects but answers nothing in 0.5 s (busy), gone or refusing (refused)
  alive   its process is there
  windows its mapped top-level windows by title (a "Save changes?" question among them)
  file:<name>  size and mtime of each watched document (a save written)
  hub     the hub log's new "kicad embed" lines (events, ended, ...), as they come
It answers nothing itself unless asked: --answer-at S --answer-key KEY focuses the question window (a window that came after the start, its title containing --answer-title)
S seconds after the start and presses KEY (Return: the default button); a question after it --next-s seconds after it shows. Ends --after-exit S seconds after the process is gone, or at --max S.
usage: quit_sampler.py --sock S --pid P --ipc gui_ipc_probe.py --driver x11_embed_driver.py --xinput xinput.py --hub-log F
       [--file F ...] [--answer-at S --answer-title T --answer-key K] [--max S] [--after-exit S]"""
import argparse, json, os, subprocess, sys, time

ap = argparse.ArgumentParser()
ap.add_argument("--sock"); ap.add_argument("--pid", type=int); ap.add_argument("--ipc"); ap.add_argument("--driver"); ap.add_argument("--xinput")
ap.add_argument("--hub-log"); ap.add_argument("--file", action="append", default=[]); ap.add_argument("--answer-at", type=float, default=-1)
ap.add_argument("--answer-title", default="save"); ap.add_argument("--answer-key", default="Return"); ap.add_argument("--max", type=float, default=90)
ap.add_argument("--after-exit", type=float, default=3); ap.add_argument("--next-s", type=float, default=2)
a = ap.parse_args()
t0 = time.monotonic()
ms = lambda: int((time.monotonic() - t0) * 1000)
def say(kind, v): print(json.dumps({"t_ms": ms(), kind: v}), flush=True)
def sock_ok():
    # "yes" answers; "busy": connects but no answer in 0.5 s (KiCad's UI thread at work: a save); "refused": gone or refusing
    if not os.path.exists(a.sock): return "refused"
    try:
        r = subprocess.run([sys.executable, a.ipc, "ping", a.sock], capture_output=True, text=True, timeout=0.5)
        if r.returncode == 0: return "yes"
        return "refused" if any(w in (r.stdout + r.stderr).lower() for w in ("refused", "no such file", "connection reset", "broken pipe")) else "no"
    except subprocess.TimeoutExpired: return "busy"
    except Exception: return "no"
def alive():
    try: os.kill(a.pid, 0); return True
    except OSError: return False
def windows():
    try:
        out = subprocess.run([sys.executable, a.driver, "list-windows", "--pid", str(a.pid)], capture_output=True, text=True, timeout=2).stdout
        ws = json.loads(out or "[]")
        return sorted({(w.get("name") or w.get("title") or "?", w.get("xid_int") or 0) for w in ws if w.get("shown")})
    except Exception: return None
def fstat(f):
    try: st = os.stat(f); return [st.st_size, int(st.st_mtime * 1000)]
    except OSError: return None
print(json.dumps({"t_ms": 0, "start_epoch_ms": int(time.time() * 1000)}), flush=True)
first = None  # the windows at the start: the question is a window that came after them
last = {}; hub_at = os.path.getsize(a.hub_log) if a.hub_log and os.path.exists(a.hub_log) else 0
done, seen = set(), {}; gone_at = None
import threading
def tail_hub():
    # the hub log read on its own every 20 ms (the sampler's own loop waits on pings): each line with its epoch, to order events
    global hub_at
    while True:
        if os.path.exists(a.hub_log):
            with open(a.hub_log, errors="replace") as h:
                h.seek(hub_at); new = h.read(); hub_at = h.tell()
            for l in new.splitlines():
                if l.startswith("kicad embed") and "publish editor" not in l and "published editor" not in l:
                    print(json.dumps({"t_ms": ms(), "epoch_ms": int(time.time() * 1000), "hub": l[:240]}), flush=True)
        time.sleep(0.02)
if a.hub_log: threading.Thread(target=tail_hub, daemon=True).start()
while ms() < a.max * 1000:
    cur = {"sock": sock_ok(), "alive": alive()}
    w = windows()
    if w is not None: cur["windows"] = [t for t, _ in w]
    if first is None and w is not None: first = {x for _, x in w}
    for f in a.file: cur["file:" + os.path.basename(f)] = fstat(f)
    for k, v in cur.items():
        if last.get(k, "unset") != v: say(k, v)
    last.update(cur)
    # the first question is left until --answer-at; each one after it (KiCad asks per editor) is answered --next-s after it shows
    for t, x in (w or []):
        if x in (first or set()) or x in done or a.answer_title.lower() not in t.lower(): continue
        seen.setdefault(x, ms())
        due = a.answer_at >= 0 and (ms() >= a.answer_at * 1000 if not done else ms() - seen[x] >= a.next_s * 1000)
        if due:
            subprocess.run([sys.executable, a.driver, "focus", str(x)], capture_output=True, timeout=3); time.sleep(0.3)
            subprocess.run([sys.executable, a.xinput, "key", a.answer_key], capture_output=True, timeout=3)
            say("answered", {"window": x, "title": t, "key": a.answer_key, "shown_ms": seen[x]}); done.add(x)
    if not cur["alive"]:
        gone_at = gone_at or ms()
        if ms() - gone_at >= a.after_exit * 1000: break
    time.sleep(0.15)
say("end", {"alive": alive()})
