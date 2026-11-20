#!/usr/bin/env python3
"""Type TEXT with real X keys (XTest) and record, in THIS process, when the first key was sent and when the last key was released.

  rail_typer.py TEXT OUT.json     writes {"first_key_ms", "last_key_ms", "chars", "ok"} when the last key is out

The times are taken by the typing process itself (never by whoever waits for it), so a later join cannot move them. Uses $DISPLAY.
"""
import ctypes
import ctypes.util
import json
import os
import sys
import time

NAMES = {" ": "space", ".": "period", ",": "comma", "/": "slash", ":": "colon", "-": "minus", "_": "underscore", "@": "at", "&": "ampersand", ">": "greater",
         "%": "percent", "$": "dollar", "#": "numbersign", "~": "asciitilde", "^": "asciicircum"}


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: rail_typer.py TEXT OUT.json", file=sys.stderr)
        return 2
    text, out = argv[1], argv[2]
    x11 = ctypes.CDLL(ctypes.util.find_library("X11"))
    xt = ctypes.CDLL(ctypes.util.find_library("Xtst"))
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XStringToKeysym.restype = ctypes.c_ulong
    x11.XStringToKeysym.argtypes = [ctypes.c_char_p]
    x11.XKeysymToKeycode.restype = ctypes.c_ubyte
    x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    xt.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    d = x11.XOpenDisplay(os.environ.get("DISPLAY", ":0").encode())
    rec = {"first_key_ms": None, "last_key_ms": None, "chars": 0, "ok": False}
    if not d:
        rec["error"] = "cannot open display"
    else:
        try:
            for ch in text:
                name = NAMES.get(ch, ch)
                kc = x11.XKeysymToKeycode(d, x11.XStringToKeysym(name.encode()))
                if not kc:
                    raise RuntimeError(f"no keycode for {name}")
                if rec["first_key_ms"] is None:
                    rec["first_key_ms"] = round(time.time() * 1000)
                xt.XTestFakeKeyEvent(d, kc, 1, 0)
                x11.XSync(d, 0)
                time.sleep(0.03)
                xt.XTestFakeKeyEvent(d, kc, 0, 0)
                x11.XSync(d, 0)
                rec["last_key_ms"] = round(time.time() * 1000)  # the last key is OUT when its release has been synced
                rec["chars"] += 1
                time.sleep(0.04)
            rec["ok"] = rec["chars"] == len(text)
        except Exception as e:  # recorded, never raised: the watcher reads the file
            rec["error"] = str(e)[:200]
    with open(out, "w", encoding="utf-8") as f:
        json.dump(rec, f)
    return 0 if rec["ok"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
