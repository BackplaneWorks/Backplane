#!/usr/bin/env python3
"""Machine linking probe: two running temp-home hubs, linked the way Bots and Setup link them (bots.invite on A,
bots.join on B), then checked from both sides (bots.peers), and again after B restarts. A protocol result, not a client one.

  link.py --a http://127.0.0.1:3811 --a-home A --b http://127.0.0.1:3812 --b-home B [--restart-b 'CMD']
"""
import argparse, json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub, check, report, finish  # noqa: E402
from settings_revoke import View  # noqa: E402


def rpc(hub, m, p=None):
    v = View(hub)
    try:
        v.next += 1
        i = v.next
        v.send({"id": i, "m": m, "p": p or {}})
        r = v.until(lambda j: str(j.get("id")) == str(i))
        return r
    finally:
        v.close()


def peers(home):
    """The live links in a hub's log (its PeerSet events, last one per id wins)."""
    rows = {}
    for line in open(os.path.join(home, "events.jsonl"), errors="replace"):
        if "PeerSet" not in line:
            continue
        try:
            e = json.loads(line)
        except ValueError:
            continue
        e = e.get("e", e) if isinstance(e, dict) else e
        if isinstance(e, dict) and e.get("$") == "PeerSet":
            rows[e["id"]] = e
    return {k: v for k, v in rows.items() if not v.get("revoked")}


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--a", required=True); a.add_argument("--a-home", required=True)
    a.add_argument("--b", required=True); a.add_argument("--b-home", required=True)
    a.add_argument("--restart-b")
    o = a.parse_args()
    A, B = (Hub(u, open(os.path.join(h, "token")).read().strip()) for u, h in ((o.a, o.a_home), (o.b, o.b_home)))
    inv = rpc(A, "bots.invite")
    if not check("A makes an invite", isinstance(inv, dict) and str(inv.get("invite", "")).startswith("bp1:"), json.dumps(inv)[:200]):
        return finish()
    j = rpc(B, "bots.join", {"invite": inv["invite"]})
    check("B joins with it", isinstance(j, dict) and bool(j.get("peer")), json.dumps(j)[:300])
    time.sleep(1)
    pa, pb = peers(o.a_home), peers(o.b_home)
    check("A holds the link to B", inv["peer"] in pa and pa[inv["peer"]].get("url", "").rstrip("/") == B.base, json.dumps(pa)[:300])
    check("B holds the link to A", inv["peer"] in pb and pb[inv["peer"]].get("url", "").rstrip("/") == A.base, json.dumps(pb)[:300])
    if o.restart_b:
        os.system(o.restart_b)
        B.wait_up(30); time.sleep(2)
        check("B still holds the link after a restart", inv["peer"] in peers(o.b_home))
    else:
        report("link survives a restart", "not run", "no --restart-b given")
    return finish()


if __name__ == "__main__":
    sys.exit(main())
