#!/usr/bin/env python3
"""Drive a real device over adb: adbui.py SERIAL tap TEXT [exact] | tapid TAG | has TEXT | type TEXT | key KEYCODE | shot FILE | wait TEXT [SECS] | texts.
Targets come from uiautomator's dump (text, content-desc, resource-id)."""
import subprocess, sys, re, time, xml.etree.ElementTree as ET
S = sys.argv[1]; cmd = sys.argv[2]; a = sys.argv[3:]
def adb(*x, binary=False): return subprocess.run(["adb", "-s", S, *x], capture_output=True, text=not binary).stdout
def dump():
    adb("shell", "uiautomator", "dump", "/sdcard/ui.xml"); x = adb("shell", "cat", "/sdcard/ui.xml")
    try: return list(ET.fromstring(x[x.index("<?xml"):]).iter("node"))
    except Exception: return []
def box(n):
    l, t, r, b = map(int, re.findall(r"\d+", n.get("bounds"))); return (l + r) // 2, (t + b) // 2
def find(ns, text, exact=False):
    for n in ns:
        for k in ("text", "content-desc"):
            v = n.get(k) or ""
            if v and ((v == text) if exact else (text in v)): return n
def tap(xy): adb("shell", "input", "tap", str(xy[0]), str(xy[1]))
if cmd in ("tap", "has", "wait"):
    secs = float(a[1]) if cmd == "wait" and len(a) > 1 else 0; end = time.time() + secs
    while True:
        n = find(dump(), a[0], "exact" in a)
        if n is not None or time.time() >= end: break
        time.sleep(1)
    if cmd == "has": sys.exit(0 if n is not None else 1)
    if cmd == "wait": sys.exit(0 if n is not None else 1)
    if n is None: sys.exit("no node with " + a[0])
    tap(box(n)); print("tapped", a[0], box(n))
elif cmd == "tapedit":
    for n in dump():
        if "EditText" in (n.get("class") or ""): tap(box(n)); print("tapped edit", box(n)); break
    else: sys.exit("no edit")
elif cmd == "tapid":
    for n in dump():
        if (n.get("resource-id") or "").endswith(a[0]): tap(box(n)); print("tapped", a[0], box(n)); break
    else: sys.exit("no id " + a[0])
elif cmd == "type": adb("shell", "input", "text", a[0].replace(" ", "%s").replace("&", "\\&").replace(">", "\\>").replace("%%", "%25"))
elif cmd == "key": adb("shell", "input", "keyevent", a[0])
elif cmd == "shot": open(a[0], "wb").write(adb("exec-out", "screencap", "-p", binary=True)); print(a[0])
elif cmd == "texts": print("\n".join(sorted({(n.get("text") or n.get("content-desc") or "") for n in dump()} - {""})))
