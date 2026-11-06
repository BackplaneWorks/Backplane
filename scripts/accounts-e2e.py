#!/usr/bin/env python3
"""Drives the account menu and a failover with real XTest keys and judges them from the hub's log and the stand-in's record. See accounts-e2e.sh."""
import ctypes, json, os, subprocess, sys, time
R, DISP = sys.argv[1], sys.argv[2]
x11, xtst = ctypes.CDLL("libX11.so.6"), ctypes.CDLL("libXtst.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p; x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XStringToKeysym.restype = ctypes.c_ulong; x11.XStringToKeysym.argtypes = [ctypes.c_char_p]
x11.XKeysymToKeycode.restype = ctypes.c_ubyte; x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
x11.XFlush.argtypes = [ctypes.c_void_p]; xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
d = x11.XOpenDisplay(DISP.encode()); assert d, "no display"
def code(k): return x11.XKeysymToKeycode(d, x11.XStringToKeysym(k.encode()))
def raw(c, down): xtst.XTestFakeKeyEvent(d, c, 1 if down else 0, 0); x11.XFlush(d); time.sleep(0.03)
def chord(*ks):
    cs = [code(k) for k in ks]
    for c in cs: raw(c, True)
    for c in reversed(cs): raw(c, False)
    time.sleep(0.25)
NAMES = {" ": "space", "-": "minus", ".": "period"}
def typed(t):
    for ch in t: chord(NAMES.get(ch, ch))
def shot(name): subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "x11grab", "-i", DISP, "-frames:v", "1", f"{R}/shots/{name}.png"], check=False)
def log(): return [json.loads(l) for l in open(R + "/home/events.jsonl") if l.strip()]
def turns():
    try: return [json.loads(l) for l in open(R + "/rec/turns.jsonl")]
    except OSError: return []
def wait(f, what, s=25):
    end = time.time() + s
    while time.time() < end:
        v = f()
        if v: return v
        time.sleep(0.15)
    shot("timeout"); print("TIMED OUT:", what); print(json.dumps(result, indent=1)); sys.exit(1)
def setting(k): v = [e for e in log() if e.get("$") == "SettingSet" and e["key"] == k]; return v[-1]["value"] if v else ""
def inventory(): return json.loads(setting("accounts.inventory") or "{}")
def lines(): return [e["summary"] for e in log() if e.get("$") == "ActivityLogged" and e.get("thread") == "t1"]
def turn(): c = [e["state"] for e in log() if e.get("$") == "TurnChanged" and e["thread"] == "t1"]; return c[-1] if c else ""
result = {"steps": []}
def ok(step, cond, why=""):
    result["steps"].append({"step": step, "ok": bool(cond), "why": str(why)[:400]}); print(("ok  " if cond else "FAIL"), step, "" if cond else why, flush=True)
time.sleep(9); shot("00-started")                         # the window drew under lavapipe
chord("Control_L", "1"); time.sleep(2.5); shot("01-thread")   # Ctrl+1: the project's first thread
chord("Control_L", "Shift_L", "a")                            # the chord, from wherever the keyboard is
ids = wait(lambda: {p["label"]: p for p in inventory().get("profiles", []) if p["state"] == "connected"} if len([p for p in inventory().get("profiles", []) if p["state"] == "connected"]) == 2 else None, "opening the menu rechecks the saved sign-ins")
ok("opening the menu rechecked both sign-ins", True); time.sleep(1.2); shot("02-menu-open")
n = len(log()); chord("3")                                   # row 3: Personal
b = wait(lambda: setting("account.thread.t1.claude"), "the digit picks the account")
ok("digit 3 picked Personal", json.loads(b)["id"] == ids["Personal"]["id"], b)
c = json.loads(setting("account.chosen.t1.claude") or "{}"); ok("the pick is recorded as the person's", c.get("by") == "person", c)
ok("3 did not reach the rail", not any(e.get("$") == "SettingSet" and "view" in e["key"] and e.get("value") == "3d" for e in log()[n:]))
time.sleep(0.8); shot("03-picked")
chord("Control_L", "Shift_L", "a"); time.sleep(0.8); shot("04-menu-again"); chord("Escape"); time.sleep(0.5); shot("05-menu-shut")
typed("hello one"); chord("Return")                          # the keyboard is back in the message box
wait(lambda: len(turns()) >= 1, "the typed message ran"); t = turns()[0]
ok("Esc gave the keyboard back and the message ran on Personal", t["who"] == "p@x.com" and "hello one" in t["text"], t)
wait(lambda: turn() == "completed", "first turn ends")
reset = int(time.time()) + 3600
open(f"{R}/rec/mode.p@x.com", "w").write(f"limit:{reset}")
typed("second request"); chord("Return")
wait(lambda: len(turns()) >= 3, "the limit moved the turn to another account", 30); wait(lambda: turn() == "completed", "the turn went on to its end")
ts = turns(); t = ts[-1]
ok("the turn ran out on Personal and went on under Work", (ts[1]["who"], t["who"]) == ("p@x.com", "w@x.com"), [x["who"] for x in ts])
ok("the context survived", all(s in t["text"] for s in ["hello one", "answered by p@x.com", "second request", "PARTIAL-p@x.com"]), t["text"][:400])
sw = [l for l in lines() if l.startswith("Switched to ")]; ok("the timeline says so", sw and sw[-1].startswith("Switched to Work: Personal ran out (resets "), lines())
ok("no turn reads as failed", not any(e.get("$") == "TurnChanged" and e["state"] == "failed" for e in log()))
time.sleep(1); shot("06-switched")
chord("Control_L", "Shift_L", "a"); time.sleep(1.2); shot("07-menu-held"); chord("Escape")
result["switch"] = sw[-1] if sw else ""; result["turns"] = [{"who": x["who"], "resume": x["resume"]} for x in ts]
json.dump(result, open(R + "/result.json", "w"), indent=1)
bad = [s for s in result["steps"] if not s["ok"]]; print("PASS" if not bad else f"{len(bad)} FAILED"); sys.exit(1 if bad else 0)
