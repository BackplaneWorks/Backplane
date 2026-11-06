#!/usr/bin/env python3
# usage: android-ui.py dump|find TEXT|has TEXT|count TEXT  (reads the emulator's UI via uiautomator)
import subprocess, sys, re, os
import xml.etree.ElementTree as ET
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
emu = ROOT + "/scripts/android-emu.sh"
def dump():
    subprocess.run([emu, "adb", "shell", "uiautomator", "dump", "/sdcard/ui.xml"], capture_output=True)
    x = subprocess.run([emu, "adb", "shell", "cat", "/sdcard/ui.xml"], capture_output=True, text=True).stdout
    try: return ET.fromstring(x[x.index("<?xml"):] if "<?xml" in x else x)
    except Exception: return None
def nodes(t):
    return [] if t is None else [n for n in t.iter("node")]
def match(n, text, exact):
    for k in ("text", "content-desc"):
        v = n.get(k) or ""
        if (v == text) if exact else (text in v): return True
    return False
cmd = sys.argv[1]
t = dump()
if cmd == "dump": print(len(nodes(t)))
elif cmd in ("bounds", "rowbox"):
    # bounds TEXT [exact] [nth]  -> l t r b of the node; rowbox: of its nearest clickable ancestor
    text = sys.argv[2]; rest = sys.argv[3:]; exact = "exact" in rest; nth = next((int(x) for x in rest if x.isdigit()), 0)
    par = {c: p for p in t.iter() for c in p}
    ms = [n for n in nodes(t) if match(n, text, exact)]
    n = ms[nth]
    if cmd == "rowbox":
        while n is not None and n.get("clickable") != "true": n = par.get(n)
    print(*re.findall(r"\d+", n.get("bounds")))
elif cmd == "edit":
    # edit [nth]: centre of the nth EditText (a Compose text field shows its hint as no node, only an empty EditText)
    nth = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    es = [n for n in nodes(t) if n.get("class") == "android.widget.EditText"]
    if not es: sys.exit(1)
    b = list(map(int, re.findall(r"\d+", es[nth].get("bounds")))); print((b[0] + b[2]) // 2, (b[1] + b[3]) // 2)
elif cmd == "clickables":
    # clickables W H [tol]: bounds of clickable nodes of about that size, in document order
    w, h = int(sys.argv[2]), int(sys.argv[3]); tol = int(sys.argv[4]) if len(sys.argv) > 4 else 4
    for n in nodes(t):
        if n.get("clickable") != "true": continue
        l, tp, r, b = map(int, re.findall(r"\d+", n.get("bounds")))
        if abs((r - l) - w) <= tol and abs((b - tp) - h) <= tol: print(l, tp, r, b)
elif cmd in ("find", "has", "count"):
    text = sys.argv[2]; exact = len(sys.argv) > 3 and sys.argv[3] == "exact"
    ms = [n for n in nodes(t) if match(n, text, exact)]
    if cmd == "count": print(len(ms))
    elif cmd == "has": sys.exit(0 if ms else 1)
    elif ms:
        b = list(map(int, re.findall(r"\d+", ms[0].get("bounds"))))
        print((b[0] + b[2]) // 2, (b[1] + b[3]) // 2)
    else: sys.exit(1)
