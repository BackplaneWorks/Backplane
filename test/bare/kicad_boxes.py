#!/usr/bin/env python3
"""KiCad's top-level windows that are not its frames: a box (a message, an error) waiting on the person.

usage: kicad_boxes.py PID [FRAME_XID ...]    (DISPLAY names the X server)
Prints one line per visible window the window manager lists (_NET_CLIENT_LIST) whose _NET_WM_PID is PID and whose id is
not one of the frame ids given: "xid<TAB>WxH+X+Y<TAB>title". Exit 0 always; nothing printed when there is none.

Why: a box KiCad opens before its first frame (Windows, no GPU: "Could not use OpenGL, falling back to software rendering")
left the pane on "Starting KiCad..." with state ready, frames 0, dialogs []: nothing said a box was waiting. _NET_WM_PID is
the pid in KiCad's own namespace, the one the client's kicad report gives, so a pid from inside the box matches.
"""
import re, subprocess, sys


def run(*a):
    try:
        return subprocess.run(a, capture_output=True, text=True, timeout=10).stdout
    except Exception:
        return ""


def main():
    if len(sys.argv) < 2 or not sys.argv[1].isdigit():
        return
    pid = sys.argv[1]
    frames = set()
    for f in sys.argv[2:]:
        try: frames.add(int(f, 0))
        except ValueError: pass
    ids = re.findall(r"0x[0-9a-fA-F]+", run("xprop", "-root", "_NET_CLIENT_LIST").partition("#")[2])
    for x in ids:
        if int(x, 16) in frames:
            continue
        p = run("xprop", "-id", x, "_NET_WM_PID", "_NET_WM_NAME", "WM_NAME")
        m = re.search(r"_NET_WM_PID\(CARDINAL\) = (\d+)", p)
        if not m or m.group(1) != pid:
            continue
        info = run("xwininfo", "-id", x)
        if "IsViewable" not in info:
            continue
        g = lambda k: (re.search(k + r":\s+(-?\d+)", info) or [None, "?"])[1]
        name = re.search(r'_NET_WM_NAME\([^)]*\) = "(.*)"', p) or re.search(r'WM_NAME\([^)]*\) = "(.*)"', p)
        print(f"{x}\t{g('Width')}x{g('Height')}+{g('Absolute upper-left X')}+{g('Absolute upper-left Y')}\t{name.group(1) if name else ''}")


if __name__ == "__main__":
    main()
