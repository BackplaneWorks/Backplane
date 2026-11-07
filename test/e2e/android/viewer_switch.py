# Viewer switching on an Android phone (the person saw the previous viewer's files after a switch): pair the debug app with a hub, then
# switch fast between viewer kinds and between two boards' threads; after each switch what the viewer draws (the debug build logs the hub
# keys, tag bpview) must hold nothing of the last view, then the new view's own. Board B's 3D model should be cold (not exported yet).
# usage: HUB_TOKEN_FILE=... HUB_PORT=3821 ADB_SERIAL=... python3 -I test/e2e/android/viewer_switch.py APK "THREAD TITLE A" "THREAD TITLE B"
#   the hub listens where `adb reverse tcp:$HUB_PORT` reaches it (127.0.0.1 on this machine); wrap a run on a shared phone in its flock
import sys, time, subprocess, re, os, json, urllib.request, urllib.parse
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "pairing")); import android
apk, ta, tb = sys.argv[1:4]
O = os.environ.get("OUT", "."); PKG = android.PKG; ADB = ["adb", "-s", os.environ["ADB_SERIAL"]] if os.environ.get("ADB_SERIAL") else ["adb"]
PORT = os.environ.get("HUB_PORT", "3821")
p = android.Phone(" ".join(ADB)); fails = []
def shot(n): open(f"{O}/sw-{n}.png", "wb").write(subprocess.run(ADB + ["exec-out", "screencap", "-p"], capture_output=True).stdout)
def surface():
    # the last thing the viewer drew, as the debug build logs it (tag bpview): "<kind>: <key> + <key>" or "<kind>: nothing"
    out = subprocess.run(ADB + ["logcat", "-d", "-s", "bpview:I"], capture_output=True, text=True).stdout.strip().splitlines()
    out = [l for l in out if "bpview" in l]
    if not out: return None
    v = out[-1].split("bpview", 1)[1].split(": ", 2)[-1].strip()
    return "nothing" if v == "nothing" else "\n".join(v.split(" + "))
def tap_text(t, last=False):
    m = [n for n in p.nodes() if n.get("text") == t]
    if m: p.tap(m[-1] if last else m[0]); return True
    return False
def wait(pred, secs):
    end = time.time() + secs
    while time.time() < end:
        v = surface()
        if v is not None and pred(v): return v
        time.sleep(0.7)
    return surface()
print(subprocess.run(ADB + ["install", "-r", apk], capture_output=True, text=True).stdout.strip().splitlines()[-1])
subprocess.run(ADB + ["reverse", f"tcp:{PORT}", f"tcp:{PORT}"])
p.sh("settings put system accelerometer_rotation 0"); p.sh("settings put system user_rotation 0")
try:
    tok = open(os.environ["HUB_TOKEN_FILE"]).read().strip()
    req = urllib.request.Request(f"http://127.0.0.1:{PORT}/auth/invitations", method="POST", headers={"Authorization": "Bearer " + tok})
    link = json.loads(urllib.request.urlopen(req).read(), strict=False)["link"]
    link = re.sub(r"https?://[^/#]+", f"http://127.0.0.1:{PORT}", link, count=1).replace(urllib.parse.quote(re.match(r"https?://[^/#]+", link)[0], safe=""), urllib.parse.quote(f"http://127.0.0.1:{PORT}", safe=""))
    p.fresh(); time.sleep(2); p.open_link("bpgpui://pair?url=" + urllib.parse.quote(link, safe="")); time.sleep(4)
    b = p.button("Pair", last=True); p.tap(b) if b is not None else None; time.sleep(6)
    def open_viewer(title):
        tap_text(title); time.sleep(2.5); p.sh("input tap 878 236"); time.sleep(2)
    subprocess.run(ADB + ["logcat", "-c"])
    open_viewer(ta)
    va = wait(lambda v: v.startswith("0|"), 30); print("A board:", va)
    root_a = va.split("|")[1] if va and "|" in va else "?"
    for tab, want in [("Schematic", "1|"), ("3D", "2|"), ("Board", "0|"), ("Schematic", "1|")]:
        tap_text(tab); time.sleep(0.4); v = surface() or ""
        bad = [l for l in v.split("\n") if l != "nothing" and not l.startswith(want) and not (want == "2|" and l.startswith("0|"))]
        print(f"tab {tab}: right after: {v!r}"); bad and fails.append(f"{tab} showed {bad}")
    v = wait(lambda v: v.startswith(f"1|{root_a}|"), 30); print("last tab shows:", v)
    if not (v or "").startswith(f"1|{root_a}|"): fails.append(f"last tab Schematic shows {v!r}")
    shot("a-schematic")
    tap_text("3D"); va3 = wait(lambda v: f"2|{root_a}|" in v, 60); print("A model:", va3)
    def go(title, tab):
        p.sh("input keyevent 4"); time.sleep(0.8); p.sh("input keyevent 4"); time.sleep(1.2)
        open_viewer(title); tap_text(tab); time.sleep(0.4)
    go(tb, "3D"); v = surface() or ""; print("B 3D right after:", repr(v)); shot("b-3d-right-after")
    if f"|{root_a}|" in v: fails.append(f"B's 3D showed A: {v!r}")
    v = wait(lambda v: v != "nothing" and f"|{root_a}|" not in v and "|" in v, 60); print("B 3D then:", repr(v))
    root_b = v.split("|")[1] if v and "|" in v else "?"
    for title, other in [(ta, root_b), (tb, root_a), (ta, root_b)]:
        go(title, "Board"); v = surface() or ""; print(f"{title} board right after: {v!r}")
        if f"|{other}|" in v: fails.append(f"{title} showed the other board: {v!r}")
    v = wait(lambda v: v.startswith(f"0|{root_a}|"), 30); print("last (A) board:", repr(v))
    if not (v or "").startswith(f"0|{root_a}|"): fails.append(f"A's board not shown at the end: {v!r}")
    shot("last")
    print("FAILS:", fails or "none")
finally:
    p.sh("settings put system accelerometer_rotation 1")
sys.exit(1 if fails else 0)
