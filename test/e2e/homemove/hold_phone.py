#!/usr/bin/env python3
"""Two real temp hubs after a home move, held up for a phone (scripts/mobile/android-offer-journey.sh real): A plays the desktop, B boat-hub.
move.py's start, link and move, then a device token of A for the phone, written to WORK/phone (a link: http://127.0.0.1:3861/#token=…).
Held until WORK/quit appears; WORK/stopA stops A (the desktop asleep), WORK/startA starts it again (each file is removed once acted on).
  hold_phone.py HUB_BIN WORK
"""
import json, os, sys, time
sys.argv = sys.argv[:3]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import move as m  # noqa: E402
from proto import secret_of  # noqa: E402

WORK = m.WORK


def main():
    A, B = m.start("A"), m.start("B")
    code = m.rpc(B, "machines.code")
    m.rpc(A, "machines.pair", {"address": B.base, "code": code.get("code", "")})
    req = m.wait(lambda: (m.rpc(B, "machines.requests") or {}).get("requests"))
    m.rpc(B, "machines.approve", {"request": req[0]["id"]})
    m.wait(lambda: any(e.get("$") == "PeerSet" and not e.get("revoked") for e in m.events(A)))
    bub = m.rpc(A, "bots.create", {"name": "Bub"}).get("bot")
    m.rpc(A, "bots.uber", {"bot": bub})
    m.wait(lambda: m.uber(A) == bub, 20)
    # the phone pairs with A before the move: its own device token
    st, inv = A.invite()
    st2, enr = A.enroll(secret_of(inv.get("link", "")), "Emulator")
    assert st2 == 200 and enr.get("token"), st2
    m.rpc(A, "home.move", {"to": "hub-b"})
    done = m.wait(lambda: (lambda s: s if s.get("state") in ("done", "failed") else None)(m.rpc(A, "home.status")), 180)
    print("move:", json.dumps(done), flush=True)
    with open(os.path.join(WORK, "phone"), "w") as f:
        f.write(f"{A.base}/#token={enr['token']}")
    print("held", flush=True)
    while not os.path.exists(os.path.join(WORK, "quit")):
        for what in ("stopA", "startA"):
            p = os.path.join(WORK, what)
            if os.path.exists(p):
                os.remove(p)
                if what == "stopA": m.stop("A")
                else: m.start("A")
                print(what, flush=True)
        time.sleep(0.5)


if __name__ == "__main__":
    try:
        main()
    finally:
        for n in list(m.procs):
            m.stop(n)
