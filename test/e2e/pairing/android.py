#!/usr/bin/env python3
"""The Android app's pairing on an emulator through its real UI: uiautomator finds the controls, `input` types and taps, and the hub says whether it worked.
A simulator result: it counts for the app and the protocol, not for a real phone.

  android.py --base http://HOST:PORT --token-file T --adb 'CMD' --sockets 'CMD' [--restart 'CMD'] [--expire 'CMD'] [--device 'a real Pixel 9 (USB)'] [--fresh 'CMD'] [--pkg dev.backplane.mobile.rel --scheme bprel]

--adb is the prefix that reaches your own emulator. --sockets prints how many sockets are established to the hub from outside, run where the hub is.
"""
import argparse, os, re, shlex, subprocess, sys, time, urllib.parse, xml.etree.ElementTree as ET
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, check, finish, report, secret_of

PKG, ACT, SCHEME = "dev.backplane.mobile.gpui", "dev.backplane.mobile.DataActivity", "bpgpui"  # (--pkg, --scheme: a release side app is dev.backplane.mobile.rel, bprel)


class Phone:
    def __init__(self, adb, fresh_cmd=""): self.adb, self.fresh_cmd = shlex.split(adb), fresh_cmd
    def sh(self, cmd, timeout=30): return subprocess.run(self.adb + ["shell", cmd], capture_output=True, text=True, timeout=timeout).stdout
    def fresh(self):
        if self.fresh_cmd:  # the app resets itself in its own process (an emulator keeps each dead app process's rendering memory: scripts/android-emu.sh fresh)
            subprocess.run(self.fresh_cmd, shell=True, capture_output=True); time.sleep(1); self.launch(); return
        self.sh(f"pm clear {PKG}"); self.sh(f"pm grant {PKG} android.permission.POST_NOTIFICATIONS"); self.launch()  # (the first-run permission dialog would cover the app)
    def stored_link(self):  # its own prefs, read as the app: a debug build only (None for a release build, which run-as refuses)
        out = self.sh(f"run-as {PKG} cat shared_prefs/data.xml 2>&1")
        return None if "not debuggable" in out or "unknown package" in out else "link" in out
    def launch(self):  # (a shade left down by another run, or a system crash dialog such as a fresh emulator's "Bluetooth keeps stopping", would cover the app)
        self.sh("cmd statusbar collapse"); b = self.button("Close app")
        if b is not None: self.tap(b); time.sleep(1)
        self.sh(f"am start -W -n {PKG}/{ACT}"); time.sleep(3)
    def nodes(self):
        self.sh("uiautomator dump /sdcard/ui.xml >/dev/null 2>&1")
        try: return list(ET.fromstring(self.sh("cat /sdcard/ui.xml")).iter("node"))
        except ET.ParseError: return []
    # The pairing screen scrolls where it does not fit (a phone in landscape, the keyboard up): what is below the fold is not in a dump, so the
    # driver scrolls to it, with swipes sized from the window, as a person would.
    def scrolls(self, ns=None): return any(n.get("scrollable") == "true" and n.get("package") == PKG for n in (ns if ns is not None else self.nodes()))
    def swipe(self, up, ns=None):  # inside the scrolling view: above the keyboard (imePadding), clear of the status bar's shade
        sc = [n for n in (ns if ns is not None else self.nodes()) if n.get("scrollable") == "true" and n.get("package") == PKG]
        if not sc: return
        x1, y1, x2, y2 = map(int, re.findall(r"\d+", sc[0].get("bounds"))); x, h = (x1 + x2) // 2, y2 - y1
        a, b = y1 + int(h * 0.8), y1 + int(h * 0.2)
        self.sh(f"input swipe {x} {a if up else b} {x} {b if up else a} 300"); time.sleep(0.6)
    def top(self):
        ns = self.nodes()
        if self.scrolls(ns):
            for _ in range(3): self.swipe(False, ns)
    def bottom(self):
        ns = self.nodes()
        if self.scrolls(ns):
            for _ in range(3): self.swipe(True, ns)
    def texts(self):
        self.hide_keyboard()  # (in landscape the keyboard leaves a strip that shows neither an error line nor what is under it)
        ns = self.nodes(); out = [t for n in ns for t in (n.get("text"), n.get("content-desc")) if t]
        if self.scrolls(ns):  # the rest of the screen too, then back to the top
            self.bottom(); out += [t for n in self.nodes() for t in (n.get("text"), n.get("content-desc")) if t and t not in out]; self.top()
        return re.sub(r"(invite|code)=[^ &|]+", r"\1=<secret>", " | ".join(out))
    def tap(self, n):
        x1, y1, x2, y2 = map(int, re.findall(r"\d+", n.get("bounds")))
        self.sh(f"input tap {(x1 + x2) // 2} {(y1 + y2) // 2}")
    def field(self):
        self.hide_keyboard(); self.top()  # (with the keyboard up in landscape the field can be out of the strip)
        for _ in range(2):  # (a dump taken while a scroll settles can miss it)
            f = next((n for n in self.nodes() if n.get("class") == "android.widget.EditText"), None)
            if f is not None: return f
            time.sleep(1)
        return None
    def button(self, label, last=False):
        m = [n for n in self.nodes() if n.get("text") == label or n.get("content-desc") == label]
        return (m[-1] if last else m[0]) if m else None
    def seek(self, label):  # a control that may be below the fold
        for i in range(4):
            b = self.button(label)
            if b is not None: return b
            self.swipe(True)
        return None
    def code_fields(self):  # the address and code fields: the two after "Or the hub's address and short code", with the screen at its bottom
        self.bottom(); ns = self.nodes()
        i = next((k for k, n in enumerate(ns) if (n.get("text") or "").startswith("Or the hub's address")), None)
        return [n for n in ns[i:] if n.get("class") == "android.widget.EditText"] if i is not None else []
    def type_and_pair(self, text):
        f = self.field()
        if f is None: return False
        self.tap(f); time.sleep(0.5)
        self.sh("input text " + shlex.quote(text))
        time.sleep(1)
        b = self.seek("Pair")  # with the keyboard up, as a person taps it: dismissing it moves the layout under the tap
        if b is None: return False
        self.tap(b); time.sleep(4); return True
    def keyboard_up(self):  # the pairing screen keeps above the keyboard (imePadding): its scrolling view is then much shorter than the window
        ns = self.nodes(); sc = [n for n in ns if n.get("scrollable") == "true" and n.get("package") == PKG]
        if not ns or not sc: return False
        h = lambda n: (lambda b: b[3] - b[1])(list(map(int, re.findall(r"\d+", n.get("bounds")))))
        return h(sc[0]) < 0.7 * h(ns[0])
    def hide_keyboard(self):  # Back only while the keyboard is up: without it, Back leaves the screen
        if self.keyboard_up(): self.sh("input keyevent 4"); time.sleep(1)
    def code_and_pair(self, address, code):
        fs = self.code_fields()
        if len(fs) < 2: return False
        self.tap(fs[0]); time.sleep(0.6); self.sh("input text " + shlex.quote(address)); time.sleep(0.6)
        self.hide_keyboard(); fs = self.code_fields()  # (in landscape the keyboard leaves room for one field, and their label scrolls away)
        if len(fs) < 2: return False
        self.tap(fs[1]); time.sleep(0.6); self.sh("input text " + shlex.quote(code)); time.sleep(0.8)
        self.hide_keyboard(); self.bottom()
        b = self.seek("Pair with code")
        if b is None: self.hide_keyboard(); b = self.seek("Pair with code")
        if b is None: return False
        self.tap(b); time.sleep(4); return True
    def open_link(self, url): self.sh("am start -W -a android.intent.action.VIEW -d " + shlex.quote(url) + f" {PKG}")  # (adb shell parses once: quoted twice, the data began with a quote and no app took it); time.sleep(5)


def live(hub): return sum(1 for d in hub.devices()[1]["devices"] if not d.get("revoked"))
def sockets(cmd): return int((subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout.strip() or "0").split()[-1])
def on_pair_screen(p): return p.field() is not None and "Pair" in p.texts()
def left_pairing(p): t = p.texts(); return bool(t) and "Pair with a hub" not in t and "Re-pair this device" not in t  # (said of what is on screen, not of a field it did not find)


def main():
    global PKG, SCHEME
    a = argparse.ArgumentParser()
    for n in ("--base", "--token-file", "--adb", "--sockets"): a.add_argument(n, required=True)
    a.add_argument("--device", default="Android emulator (a simulator result)"); a.add_argument("--pkg", default=PKG); a.add_argument("--scheme", default=SCHEME); a.add_argument("--fresh", default="", help="a host command that resets the app in its process, instead of pm clear"); a.add_argument("--restart", default=""); a.add_argument("--expire", default="")
    o = a.parse_args()
    PKG, SCHEME = o.pkg, o.scheme
    hub, p = Hub(o.base, open(o.token_file).read().strip()), Phone(o.adb, o.fresh)
    print(f"route {hub.base}, {o.device}", flush=True)

    if o.fresh: p.sh(f"pm grant {PKG} android.permission.POST_NOTIFICATIONS")  # once: fresh() no longer clears it
    p.fresh()
    check("a fresh app starts at pairing", on_pair_screen(p), p.texts()[:90])
    print("   screen offers:", p.texts()[:160], flush=True)
    check("the screen offers a hub address and short code", len(p.code_fields()) == 2); p.top()

    # link, typed into the field
    n0 = live(hub); st, inv = hub.invite()
    check("link typed and Pair tapped", p.type_and_pair(inv["link"]))
    check("link: the hub lists one more live device", live(hub) == n0 + 1, p.texts()[:90])
    check("link: the app holds a socket to the hub", sockets(o.sockets) >= 1)
    check("link: the app has left pairing", left_pairing(p), p.texts()[:90])
    had = p.stored_link(); p.fresh()
    if had is None: report("a fresh start after pairing leaves no hub stored", "not run", "a release build: its stored settings cannot be read"); check("a fresh start after pairing shows pairing", on_pair_screen(p) and "Re-pair" not in p.texts(), p.texts()[:90])
    else: check("a fresh start after pairing leaves no hub stored and shows pairing", had and not p.stored_link() and on_pair_screen(p) and "Re-pair" not in p.texts(), f"stored before {had}, after {p.stored_link()}")

    # a short code
    p.fresh(); n0 = live(hub); st, inv = hub.invite()
    check("address and short code typed", p.code_and_pair(hub.base, inv["code"].lower()[:4] + "-" + inv["code"].lower()[4:]))
    check("short code pairs (lower case, with a dash)", live(hub) == n0 + 1, p.texts()[:90])
    check("short code: the app has left pairing", left_pairing(p), p.texts()[:90])
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
    p.open_link(f"{SCHEME}://pair?url=" + urllib.parse.quote(inv["link"], safe=""))
    origin = hub.base
    check(f"a {SCHEME}:// link names its hub and asks", "Pair with this hub?" in p.texts() and origin in p.texts() and live(hub) == n0, p.texts()[:110])
    b = p.button("Cancel"); p.tap(b) if b is not None else None; time.sleep(1.5)
    check("Cancel pairs nothing", live(hub) == n0 and "Pair with this hub?" not in p.texts())
    p.open_link(f"{SCHEME}://pair?url=" + urllib.parse.quote(inv["link"], safe=""))
    b = p.button("Pair", last=True); p.tap(b) if b is not None else None; time.sleep(5)  # (the question is drawn over the pairing form, whose own Pair comes first in the tree)
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
