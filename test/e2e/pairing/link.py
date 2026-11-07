#!/usr/bin/env python3
"""Machine linking probe: two running temp-home hubs, linked the way Bots and Setup link them (bots.invite on A,
bots.join on B), then checked from both sides (bots.peers), and again after B restarts. A protocol result, not a client one.

  link.py --a http://127.0.0.1:3811 --a-home A --b http://127.0.0.1:3812 --b-home B [--restart-b 'CMD']

Two ways are driven: the pasted `bp1:` invite (bots.invite / bots.join: a link nobody vouched for, unpaired), and the pairing
a person approves (machines.code on A; machines.pair on B with A's address and the code; the request waits on A with B's check
code; machines.approve on A; or machines.deny). Both hubs are told nothing else: no machine links by itself.
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


def own_settings(home):
    """peer.own.<id> -> value, the last SettingSet of each in a hub's log."""
    out = {}
    for line in open(os.path.join(home, "events.jsonl"), errors="replace"):
        if "peer.own." not in line:
            continue
        try:
            e = json.loads(line)
        except ValueError:
            continue
        e = e.get("e", e) if isinstance(e, dict) else e
        if isinstance(e, dict) and e.get("$") == "SettingSet" and str(e.get("key", "")).startswith("peer.own."):
            out[e["key"][len("peer.own."):]] = e.get("value")
    return out


def wait(fn, secs=20):
    t = time.time()
    while time.time() - t < secs:
        r = fn()
        if r:
            return r
        time.sleep(0.5)
    return None


def pairing(A, B, o):
    """A makes a code, B enters it, the request waits on A, the person approves (or denies)."""
    code = rpc(A, "machines.code")
    if not check("A makes a machine code", isinstance(code, dict) and len(str(code.get("code", ""))) == 8, json.dumps(code)[:200]):
        return
    # a code whose request is denied first (a machine already linked cannot ask again)
    code2 = rpc(A, "machines.code")
    p2 = rpc(B, "machines.pair", {"address": A.base, "code": code2.get("code", "")}) if isinstance(code2, dict) else None
    if isinstance(p2, dict) and p2.get("attempt"):
        r2 = wait(lambda: (rpc(A, "machines.requests") or {}).get("requests"))
        rpc(A, "machines.deny", {"request": r2[0]["id"]}) if r2 else None
        st2 = wait(lambda: (lambda r: r if isinstance(r, dict) and r.get("state") in ("linked", "refused") else None)(rpc(B, "machines.attempt", {"attempt": p2["attempt"]})))
        check("a denied request is refused on B", bool(st2) and st2.get("state") == "refused", json.dumps(st2)[:300])
    else:
        check("B asks again with a second code", False, json.dumps(p2)[:200])
    bad = rpc(B, "machines.pair", {"address": A.base, "code": "WRONGCOD"})
    check("a wrong code is refused", isinstance(bad, dict) and bad.get("ok") is False, json.dumps(bad)[:200])
    before = set(peers(o.a_home))
    p = rpc(B, "machines.pair", {"address": A.base, "code": code["code"]})
    if not check("B asks with the code and gets a check code", isinstance(p, dict) and len(str(p.get("check", ""))) == 6, json.dumps(p)[:200]):
        return
    req = wait(lambda: (rpc(A, "machines.requests") or {}).get("requests"))
    check("the request waits on A with B's check code", bool(req) and req[0].get("check") == p["check"], json.dumps(req)[:300])
    time.sleep(1)
    check("nothing is linked before the approval", set(peers(o.a_home)) == before and set(peers(o.b_home)) == before)
    if not req:
        return
    ok = rpc(A, "machines.approve", {"request": req[0]["id"]})
    check("A approves", isinstance(ok, dict) and ok.get("ok") is True, json.dumps(ok)[:200])
    st = wait(lambda: (lambda r: r if isinstance(r, dict) and r.get("state") in ("linked", "refused") else None)(rpc(B, "machines.attempt", {"attempt": p["attempt"]})))
    check("B links", bool(st) and st.get("state") == "linked", json.dumps(st)[:300])
    time.sleep(1)
    new_a = set(peers(o.a_home)) - before
    new_b = set(peers(o.b_home)) - before
    check("both hold the same new link", len(new_a) == 1 and new_a == new_b, f"{new_a} {new_b}")
    if new_a:
        i = next(iter(new_a))
        check("both record it as approved (trusted)", own_settings(o.a_home).get(i) == "approved" and own_settings(o.b_home).get(i) == "approved", f"{own_settings(o.a_home)} {own_settings(o.b_home)}")


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--a", required=True); a.add_argument("--a-home", required=True)
    a.add_argument("--b", required=True); a.add_argument("--b-home", required=True)
    a.add_argument("--restart-b")
    o = a.parse_args()
    A, B = (Hub(u, open(os.path.join(h, "token")).read().strip()) for u, h in ((o.a, o.a_home), (o.b, o.b_home)))
    pairing(A, B, o)
    inv = rpc(A, "bots.invite")
    if not check("A makes an invite", isinstance(inv, dict) and str(inv.get("invite", "")).startswith("bp1:"), json.dumps(inv)[:200]):
        return finish()
    j = rpc(B, "bots.join", {"invite": inv["invite"]})
    check("B joins with it", isinstance(j, dict) and bool(j.get("peer")), json.dumps(j)[:300])
    time.sleep(1)
    pa, pb = peers(o.a_home), peers(o.b_home)
    check("a pasted invite is not a pairing (unpaired until marked Mine)", own_settings(o.a_home).get(inv["peer"]) in (None, ""), json.dumps(own_settings(o.a_home))[:200])
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
