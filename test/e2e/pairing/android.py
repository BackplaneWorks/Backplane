#!/usr/bin/env python3
"""The Android app's pairing on an emulator through its real UI: uiautomator finds the controls, `input` types and taps, and the hub says whether it worked.
A simulator result: it counts for the app and the protocol, not for a real phone.

  android.py --base http://HOST:PORT --token-file T --adb 'CMD' --sockets 'CMD' [--restart 'CMD'] [--expire 'CMD']

--adb is the prefix that reaches your own emulator. --sockets prints how many sockets are established to the hub from outside, run where the hub is.
"""
import argparse, os, re, shlex, subprocess, sys, time, urllib.parse, xml.etree.ElementTree as ET
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, check, finish, report, secret_of

PKG, ACT = "dev.backplane.mobile.gpui", "dev.backplane.mobile.DataActivity"


class Phone:
    def __init__(self, adb): self.adb = shlex.split(adb)
    def sh(self, cmd, timeout=30): return subprocess.run(self.adb + ["shell", cmd], capture_output=True, text=True, timeout=timeout).stdout
    def fresh(self): self.sh(f"pm clear {PKG}"); self.sh(f"pm grant {PKG} android.permission.POST_NOTIFICATIONS"); self.launch()  # (the first-run permission dialog would cover the app)
    def launch(self): self.sh(f"am start -W -n {PKG}/{ACT}"); time.sleep(3)
    def nodes(self):
        self.sh("uiautomator dump /sdcard/ui.xml >/dev/null 2>&1")
        try: return list(ET.fromstring(self.sh("cat /sdcard/ui.xml")).iter("node"))
        except ET.ParseError: return []
    def texts(self): return re.sub(r"(invite|code)=[^ &|]+", r"\1=<secret>", " | ".join(t for n in self.nodes() for t in (n.get("text"), n.get("content-desc")) if t))
    def tap(self, n):
        x1, y1, x2, y2 = map(int, re.findall(r"\d+", n.get("bounds")))
        self.sh(f"input tap {(x1 + x2) // 2} {(y1 + y2) // 2}")
    def field(self): return next((n for n in self.nodes() if n.get("class") == "android.widget.EditText"), None)
    def button(self, label): return next((n for n in self.nodes() if n.get("text") == label or n.get("content-desc") == label), None)
    def type_and_pair(self, text):
        f = self.field()
        if f is None: return False
        self.tap(f); time.sleep(0.5)
        self.sh("input text " + shlex.quote(text))
        time.sleep(1)
        b = self.button("Pair")  # with the keyboard up, as a person taps it: dismissing it moves the layout under the tap
        if b is None: return False
        self.tap(b); time.sleep(4); return True
    def code_and_pair(self, address, code):
        fs = [n for n in self.nodes() if n.get("class") == "android.widget.EditText"]
        if len(fs) < 2: return False
        self.tap(fs[1]); time.sleep(0.6); self.sh("input text " + shlex.quote(address)); time.sleep(0.6)
        fs = [n for n in self.nodes() if n.get("class") == "android.widget.EditText"]
        self.tap(fs[2]); time.sleep(0.6); self.sh("input text " + shlex.quote(code)); time.sleep(0.8)
        b = self.button("Pair with code")
        if b is None: self.sh("input keyevent 4"); time.sleep(1.2); b = self.button("Pair with code")
        if b is None: return False
        self.tap(b); time.sleep(4); return True
    def open_link(self, url): self.sh("am start -W -a android.intent.action.VIEW -d " + shlex.quote(shlex.quote(url)) + f" {PKG}"); time.sleep(5)


def live(hub): return sum(1 for d in hub.devices()[1]["devices"] if not d.get("revoked"))
def sockets(cmd): return int((subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout.strip() or "0").split()[-1])
def on_pair_screen(p): return p.field() is not None and "Pair" in p.texts()


def main():
    a = argparse.ArgumentParser()
    for n in ("--base", "--token-file", "--adb", "--sockets"): a.add_argument(n, required=True)
    a.add_argument("--restart", default=""); a.add_argument("--expire", default="")
    o = a.parse_args()
    hub, p = Hub(o.base, open(o.token_file).read().strip()), Phone(o.adb)
    print(f"route {hub.base}, Android emulator (a simulator result)", flush=True)

    p.fresh()
    check("a fresh app starts at pairing", on_pair_screen(p), p.texts()[:90])
    print("   screen offers:", p.texts()[:160], flush=True)
    check("the screen offers a hub address and short code", sum(1 for n in p.nodes() if n.get("class") == "android.widget.EditText") == 3)

    # link, typed into the field
    n0 = live(hub); st, inv = hub.invite()
    check("link typed and Pair tapped", p.type_and_pair(inv["link"]))
    check("link: the hub lists one more live device", live(hub) == n0 + 1, p.texts()[:90])
    check("link: the app holds a socket to the hub", sockets(o.sockets) >= 1)
    check("link: the app has left pairing", not on_pair_screen(p), p.texts()[:90])

    # a short code
    p.fresh(); n0 = live(hub); st, inv = hub.invite()
    check("address and short code typed", p.code_and_pair(hub.base, inv["code"].lower()[:4] + "-" + inv["code"].lower()[4:]))
    check("short code pairs (lower case, with a dash)", live(hub) == n0 + 1, p.texts()[:90])
    check("short code: the app has left pairing", not on_pair_screen(p), p.texts()[:90])
    p.fresh(); st, inv = hub.invite(); n0 = live(hub)
    p.type_and_pair(inv["code"])
    check("a bare code is not taken for a hub address", on_pair_screen(p) and "address with the short code" in p.texts(), p.texts()[:120])

    # refusals: the words on screen
    time.sleep(61)
    p.fresh(); p.type_and_pair(f"{hub.base}/pair#code=ZZZZ2222")
    check("wrong code says so", "That code is not correct" in p.texts(), p.texts()[:110])
    st, inv = hub.invite(); hub.enroll(inv["code"], "used by the probe")
    p.fresh(); p.type_and_pair(inv["link"])
    check("used link says so", "already been used" in p.texts(), p.texts()[:110])
    st, inv = hub.invite(); hub.revoke_invite(inv["id"])
    p.fresh(); p.type_and_pair(inv["link"])
    check("revoked link says so", "was revoked" in p.texts(), p.texts()[:110])
    if o.expire:
        st, inv = hub.invite()
        subprocess.run(o.expire, shell=True, capture_output=True); hub.wait_up()
        p.fresh(); p.type_and_pair(inv["link"])
        check("expired link says so (expiresAt moved back in the registry, hub restarted)", "expired" in p.texts(), p.texts()[:110])
    else: report("expired link", "not run", "no --expire")

    # a link from outside the app
    time.sleep(61)
    p.fresh(); n0 = live(hub); st, inv = hub.invite()
    p.open_link("bpgpui://pair?url=" + urllib.parse.quote(inv["link"], safe=""))
    origin = hub.base
    check("a bpgpui:// link names its hub and asks", "Pair with this hub?" in p.texts() and origin in p.texts() and live(hub) == n0, p.texts()[:110])
    b = p.button("Cancel"); p.tap(b) if b is not None else None; time.sleep(1.5)
    check("Cancel pairs nothing", live(hub) == n0 and "Pair with this hub?" not in p.texts())
    p.open_link("bpgpui://pair?url=" + urllib.parse.quote(inv["link"], safe=""))
    b = p.button("Pair"); p.tap(b) if b is not None else None; time.sleep(5)
    check("confirming the question pairs", live(hub) == n0 + 1, p.texts()[:90])

    # revoke while connected, re-pair
    ids = [d for d in hub.devices()[1]["devices"] if not d.get("revoked")]
    n = sockets(o.sockets)
    for d in ids: hub.revoke_device(d["id"])
    time.sleep(6)
    check("revoke ends the app's socket", sockets(o.sockets) < max(n, 1), f"{n} before")
    print("   after revoke:", p.texts()[:160], flush=True)
    check("a revoked app is at Re-pair", "Re-pair this device" in p.texts(), p.texts()[:110])
    n0 = live(hub); st, inv = hub.invite()
    p.type_and_pair(inv["link"])
    check("re-pair after revoke", live(hub) == n0 + 1, p.texts()[:90])
    check("re-paired: the app holds a socket", sockets(o.sockets) >= 1)

    # a hub restart
    if o.restart:
        subprocess.run(o.restart, shell=True, capture_output=True); hub.wait_up()
        end, n = time.time() + 40, 0
        while time.time() < end and n < 1: time.sleep(2); n = sockets(o.sockets)
        check("the app reconnects after a hub restart", n >= 1, f"{n} socket(s)")
    else: report("hub restart", "not run", "no --restart")
    return finish()


if __name__ == "__main__":
    sys.exit(main())
