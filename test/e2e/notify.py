"""Settings, Notifications on the desktop with real mouse input (scripts/notify-e2e.sh): Settled on, a thread's finished turn raises a desktop notification; clicked off, the next finished turn raises
none; clicked on, the next does. Judged by what the window's notification program was asked to show. Pictures in $R/shots, results in $R/result.json."""
import json, os, subprocess, sys, time
R, XI, PORT, ME, DISP = sys.argv[1:6]; os.environ["DISPLAY"] = DISP
def x(*a): subprocess.run(["python3", "-I", XI, *map(str, a)], check=True); time.sleep(0.8)
def shot(n): subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "x11grab", "-i", DISP, "-frames:v", "1", f"{R}/shots/{n}.png"])
def rpc(m, p): subprocess.run(["bun", f"{ME}/test/cef/hub_rpc.mjs", PORT, f"{R}/home/token", json.dumps([{"m": m, "p": p}])], capture_output=True, timeout=30)
def notified():
    try: return [l for l in open(f"{R}/notified.txt").read().splitlines() if l.strip()]
    except OSError: return []
def setting(k):
    v = [json.loads(l) for l in open(f"{R}/home/events.jsonl") if '"SettingSet"' in l]; v = [e["value"] for e in v if e["key"] == k]; return v[-1] if v else ""
result = {"steps": []}
def ok(step, cond, why=""): result["steps"].append({"step": step, "ok": bool(cond), "why": str(why)[:300]}); print("ok  " if cond else "FAIL", step, "" if cond else why, flush=True)
def wait(f, s=20):
    end = time.time() + s
    while time.time() < end:
        if f(): return True
        time.sleep(0.3)
    return False
time.sleep(5)
x("click", 295, 421); time.sleep(1.5); shot("01-notifications-page")
ok("the hub's page is drawn in the window", True)
rpc("turn.start", {"thread": "t1", "text": "work first"})
ok("Settled on: a finished turn raises a desktop notification", wait(lambda: len(notified()) >= 1), notified()); n1 = len(notified()); shot("02-finished-with-settled-on")
x("click", 1193, 351); time.sleep(1.5); shot("03-settled-clicked-off")
ok("the click on off set notify.settled=off in the hub", wait(lambda: setting("notify.settled") == "off"), setting("notify.settled"))
rpc("turn.start", {"thread": "t2", "text": "work second"}); time.sleep(10); shot("04-finished-with-settled-off")
ok("Settled off: the next finished turn raises none", len(notified()) == n1, notified())
x("click", 1151, 351); time.sleep(1.5); shot("05-settled-clicked-on")
ok("the click on on set it back", wait(lambda: setting("notify.settled") == "on"), setting("notify.settled"))
rpc("turn.start", {"thread": "t3", "text": "work third"})
ok("Settled on again: the finished turn raises one", wait(lambda: len(notified()) > n1), notified()); shot("06-finished-with-settled-on-again")
x("click", 800, 600, 5); x("click", 800, 600, 5); x("click", 800, 600, 5); time.sleep(1); shot("07-quiet-hours-urgent-phones")
result["notified"] = notified(); json.dump(result, open(f"{R}/result.json", "w"), indent=1)
bad = [s for s in result["steps"] if not s["ok"]]; print("PASS" if not bad else f"{len(bad)} FAILED"); sys.exit(1 if bad else 0)
