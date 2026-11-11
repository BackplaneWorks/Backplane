#!/usr/bin/env python3
"""The iOS data app's pairing on a simulator. There are no tap tools on the Mac: a debug hook calls what the form's Pair calls, BACKPLANE_ASK answers the question,
an outside link is offered through `simctl openurl` (iOS then asks its own "Open in …?", which no tap answers here) and driven through
BACKPLANE_OPEN_URL, the same route() as onOpenURL. So this proves the app and the protocol on a simulator, not its buttons and not a real phone.

  ios_sim.py --base http://HOST:PORT --token-file T --sim UDID --app PATH --sockets 'CMD' [--restart 'CMD'] [--expire 'CMD'] [--mac hs-mac-mini]
"""
import argparse, os, re, shlex, subprocess, sys, time, urllib.parse
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, check, finish, report

BID = "dev.backplane.gpui"
PRE = "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; "


class Sim:
    def __init__(self, mac, udid, app): self.mac, self.u, self.app = mac, udid, app
    def sh(self, cmd, timeout=60): return subprocess.run(["ssh", "-o", "BatchMode=yes", self.mac, PRE + cmd], capture_output=True, text=True, timeout=timeout).stdout
    def stop(self): self.sh(f"xcrun simctl terminate {self.u} {BID}")
    def fresh(self):
        self.stop(); self.sh(f"xcrun simctl uninstall {self.u} {BID}; xcrun simctl keychain {self.u} reset; xcrun simctl install {self.u} {shlex.quote(self.app)}")
    def launch(self, **env):
        self.stop()
        e = " ".join(f"SIMCTL_CHILD_{k}={shlex.quote(v)}" for k, v in env.items())
        self.mark = time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime(float(self.sh("date +%s").strip() or time.time())))
        self.sh(f"{e} xcrun simctl launch {self.u} {BID}")
    def log(self):
        out = self.sh(f"xcrun simctl spawn {self.u} log show --last 2m --style compact --predicate 'eventMessage CONTAINS \"BPPAIR\"'")
        return re.sub(r"(invite|code)=[^ &]+", r"\1=<secret>", "\n".join(l.split("BPPAIR", 1)[1].strip() for l in out.splitlines() if "BPPAIR" in l and "Filtering the log" not in l and "log run noninteractively" not in l))
    def springboard(self): return self.sh(f"xcrun simctl spawn {self.u} log show --last 30s --style compact --predicate 'process == \"SpringBoard\" AND eventMessage CONTAINS \"Open in\"'")
    def open(self, url): self.sh(f"xcrun simctl openurl {self.u} {shlex.quote(url)}")


def live(hub): return sum(1 for d in hub.devices()[1]["devices"] if not d.get("revoked"))
def sockets(cmd): return int((subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout.strip() or "0").split()[-1])
def last(text): return text.strip().splitlines()[-1][:110] if text.strip() else ""


def main():
    a = argparse.ArgumentParser()
    for n in ("--base", "--token-file", "--sim", "--app", "--sockets"): a.add_argument(n, required=True)
    a.add_argument("--restart", default=""); a.add_argument("--expire", default=""); a.add_argument("--mac", default="hs-mac-mini")
    o = a.parse_args()
    hub, s = Hub(o.base, open(o.token_file).read().strip()), Sim(o.mac, o.sim, o.app)
    print(f"route {hub.base}, iOS simulator (a simulator result, hook-driven: no taps)", flush=True)

    def pairs(text, wait=7): s.fresh(); s.launch(BACKPLANE_PAIR_TEXT=text); time.sleep(wait)
    # link
    n0 = live(hub); st, inv = hub.invite(); pairs(inv["link"])
    check("link: the hub lists one more live device", live(hub) == n0 + 1, last(s.log()))
    check("link: the app holds a socket to the hub", sockets(o.sockets) >= 1)
    # the code form makes this link
    n0 = live(hub); st, inv = hub.invite(); pairs(f"{hub.base}/pair#code={inv['code']}")
    check("short code (as Connect with code makes it) pairs", live(hub) == n0 + 1, last(s.log()))
    st, inv = hub.invite(); pairs(inv["code"], 5)
    check("a bare code is not taken for a hub address", "address with the short code" in s.log(), last(s.log()))
    # refusals
    time.sleep(61)
    pairs(f"{hub.base}/pair#code=ZZZZ2222"); check("wrong code says so", "That code is not correct" in s.log(), last(s.log()))
    st, inv = hub.invite(); hub.enroll(inv["code"], "used by the probe"); pairs(inv["link"])
    check("used link says so", "already been used" in s.log(), last(s.log()))
    st, inv = hub.invite(); hub.revoke_invite(inv["id"]); pairs(inv["link"])
    check("revoked link says so", "was revoked" in s.log(), last(s.log()))
    if o.expire:
        st, inv = hub.invite(); subprocess.run(o.expire, shell=True, capture_output=True); hub.wait_up(); pairs(inv["link"])
        check("expired link says so (expiresAt moved back in the registry, hub restarted)", "expired" in s.log(), last(s.log()))
    else: report("expired link", "not run", "no --expire")
    # a link from outside
    time.sleep(61)
    n0 = live(hub); st, inv = hub.invite(); url = "backplane-gpui://pair?url=" + urllib.parse.quote(inv["link"], safe="")
    # iOS asks first ("Open in “Backplane”?", SpringBoard's own alert) and a simulator has no taps to answer it: the link is offered to this app,
    # then the app's route is driven by its debug hook (BACKPLANE_OPEN_URL, the same route() as onOpenURL)
    s.fresh(); s.launch(); time.sleep(3); s.open(url); time.sleep(3)
    check("the system offers the link to this app (its own Open in … question)", "Open in “Backplane”?" in s.springboard(), "no alert in SpringBoard's log")
    report("iOS's Open in … answered", "not run", "a simulator has no taps; the person's TestFlight check")
    s.fresh(); s.launch(BACKPLANE_ASK="cancel", BACKPLANE_OPEN_URL=url); time.sleep(7)
    check("an outside link names its hub and asks", f"ask {hub.base}" in s.log(), last(s.log()))
    check("Cancel pairs nothing", live(hub) == n0)
    s.launch(BACKPLANE_ASK="yes", BACKPLANE_OPEN_URL=url); time.sleep(9)
    check("confirming the question pairs", live(hub) == n0 + 1, last(s.log()))
    # revoke while connected, re-pair
    n = sockets(o.sockets)
    for d in hub.devices()[1]["devices"]:
        if not d.get("revoked"): hub.revoke_device(d["id"])
    time.sleep(8)
    check("revoke ends the app's socket", n >= 1 and sockets(o.sockets) == 0, f"{n} before")
    check("a revoked app is at Re-pair", "screen re-pair" in s.log(), last(s.log()))
    n0 = live(hub); st, inv = hub.invite(); s.launch(BACKPLANE_PAIR_TEXT=inv["link"]); time.sleep(7)
    check("re-pair after revoke", live(hub) == n0 + 1, last(s.log()))
    check("re-paired: the app holds a socket", sockets(o.sockets) >= 1)
    if o.restart:
        subprocess.run(o.restart, shell=True, capture_output=True); hub.wait_up()
        end, n = time.time() + 40, 0
        while time.time() < end and n < 1: time.sleep(2); n = sockets(o.sockets)
        check("the app reconnects after a hub restart", n >= 1, f"{n} socket(s)")
    else: report("hub restart", "not run", "no --restart")
    s.stop()
    return finish()


if __name__ == "__main__":
    sys.exit(main())
