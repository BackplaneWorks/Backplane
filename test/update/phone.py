#!/usr/bin/env python3
"""Drive the Android app on one device by what is on screen (uiautomator): test/update/feed.sh's phone case.

    phone.py SERIAL tap TEXT [exact] [nth]   tap the nth node showing TEXT (text or content-desc); -1 is the last
    phone.py SERIAL has TEXT [exact]         exit 0 when shown
    phone.py SERIAL seek TEXT [exact]        scroll up until TEXT shows
    phone.py SERIAL text PREFIX              print the first text starting with PREFIX
    phone.py SERIAL shot FILE                screenshot
Always `adb -s SERIAL`: another device may be attached.
"""
import re, subprocess, sys, time
import xml.etree.ElementTree as ET

serial, cmd, args = sys.argv[1], sys.argv[2], sys.argv[3:]
def adb(*a, **k): return subprocess.run(["adb", "-s", serial, *a], capture_output=True, **k)
def dump():
    adb("shell", "uiautomator", "dump", "/sdcard/bp-ui.xml")
    x = adb("shell", "cat", "/sdcard/bp-ui.xml", text=True).stdout
    try: return list(ET.fromstring(x[x.index("<?xml"):]).iter("node"))
    except Exception: return []
def shown(n, t, exact):
    return any(((n.get(k) or "") == t) if exact else (t in (n.get(k) or "")) for k in ("text", "content-desc"))
if cmd == "shot":
    open(args[0], "wb").write(adb("exec-out", "screencap", "-p").stdout); sys.exit(0)
t = args[0]; exact = "exact" in args[1:]; nth = next((int(a) for a in args[1:] if a.lstrip("-").isdigit()), 0)  # -1: the last one (a dialog is drawn after the page under it)
ns = [n for n in dump() if shown(n, t, exact)]
if cmd == "seek":   # scroll the page up until TEXT shows (a long Settings page): Compose leaves what is off screen out of the tree
    for _ in range(6):
        if ns: sys.exit(0)
        w, h = (int(x) for x in re.findall(r"\d+", adb("shell", "wm", "size", text=True).stdout)[-2:])
        adb("shell", "input", "swipe", str(w // 2), str(h * 3 // 4), str(w // 2), str(h // 4), "300"); time.sleep(0.8)
        ns = [n for n in dump() if shown(n, t, exact)]
    sys.exit(0 if ns else 1)
if cmd == "has": sys.exit(0 if ns else 1)
if cmd == "text":
    for n in dump():
        v = n.get("text") or ""
        if v.startswith(t): print(v); sys.exit(0)
    sys.exit(1)
if cmd == "tap":
    if not ns or nth >= len(ns) or -nth > len(ns): print(f"no '{t}' on screen", file=sys.stderr); sys.exit(1)
    l, tp, r, b = map(int, re.findall(r"\d+", ns[nth].get("bounds")))
    adb("shell", "input", "tap", str((l + r) // 2), str((tp + b) // 2)); time.sleep(1.2); sys.exit(0)
sys.exit(2)
