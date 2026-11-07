#!/usr/bin/env python3
"""Drive the Android app on one device by what is on screen (uiautomator): test/update/feed.sh's phone case.

    phone.py SERIAL tap TEXT [exact] [nth]   tap the nth node showing TEXT (text or content-desc); -1 is the last
    phone.py SERIAL has TEXT [exact]         exit 0 when shown
    phone.py SERIAL seek TEXT [exact]        scroll down, then up, until TEXT shows
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
    for i in range(12):   # down the page first, then back up it
        if ns: sys.exit(0)
        # the screen as drawn now (the phone may be in landscape; `wm size` gives the portrait numbers)
        w, h = (int(x) for x in re.findall(r"\d+", (dump() or [ET.Element("node", bounds="[0,0][1080,1080]")])[0].get("bounds"))[2:4])
        a, b = (h * 3 // 4, h // 4) if i < 4 else (h // 4, h * 3 // 4)
        adb("shell", "input", "swipe", str(w // 2), str(a), str(w // 2), str(b), "300"); time.sleep(0.8)
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
