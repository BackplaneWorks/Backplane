#!/usr/bin/env python3
"""The home move end to end (docs/BOAT-HUB.md) on two temp-home hubs and a simulated phone. Nothing of the person's is touched.

  move.py HUB_BIN WORK

A plays the desktop, B the always-on boat-hub, both on 127.0.0.1 with --no-tailscale and an empty stand-in agents folder. They are linked by an
approved pairing (machines.code on B, machines.pair from A, machines.approve on B). A gets an uberbot (Bub), a second bot, a room, a memory and
a webhook; a phone pairs with A (an invitation and enrollment: its own device token) and registers for push. B becomes the ring's front door
(ring.front.use). Then, on A: home.move to B, checked; the phone's token on B; the webhook through B with A stopped; home.move back, checked.
Exit 1 when a check failed. Python 3.9, standard library only.
"""
import json, os, subprocess, sys, time, urllib.request
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../pairing"))
from proto import Hub, Socket, check, finish, secret_of  # noqa: E402
from settings_revoke import View  # noqa: E402

BIN, WORK = sys.argv[1], sys.argv[2]
PORTS = {"A": 3861, "B": 3862}
procs = {}


def start(name):
    home = os.path.join(WORK, name)
    os.makedirs(home, exist_ok=True)
    os.makedirs(os.path.join(WORK, "agents"), exist_ok=True)
    # the move's readiness check asks the target's `claude auth status`: a stand-in that is signed in (everywhere.py writes its own)
    cl = os.path.join(WORK, "agents", "claude")
    if not os.path.exists(cl):
        with open(cl, "w") as f:
            f.write("#!/bin/sh\n[ \"$1\" = auth ] && { echo '{\"loggedIn\":true,\"email\":\"standin@example.com\"}'; exit 0; }\nexit 1\n")
        os.chmod(cl, 0o755)
    env = dict(os.environ, BACKPLANE_AGENTS=os.path.join(WORK, "agents"), BACKPLANE_NO_UPDATE="1", BACKPLANE_NAME=f"hub-{name.lower()}",
               BACKPLANE_BOTS_DIR=os.path.join(home, "botsdir"), BACKPLANE_UBER_GRACE="600", BACKPLANE_MOVE_PAUSE_MS=os.environ.get("BACKPLANE_MOVE_PAUSE_MS", ""), HOME=os.path.join(WORK, f"home-{name}"))
    for k in ("DISPLAY", "WAYLAND_DISPLAY", "BACKPLANE_URL", "BACKPLANE_HOOK_URL", "BACKPLANE_HTTPS_PORT"):
        env.pop(k, None)
    os.makedirs(env["HOME"], exist_ok=True)
    log = open(os.path.join(WORK, f"{name}.log"), "a")
    procs[name] = subprocess.Popen([BIN, "--home", home, "--port", str(PORTS[name]), "--host", "127.0.0.1", "--no-tailscale", "--foreground", "--headless"],
                                   env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    tok = os.path.join(home, "token")
    for _ in range(100):
        if os.path.exists(tok):
            break
        time.sleep(0.1)
    h = Hub(f"http://127.0.0.1:{PORTS[name]}", open(tok).read().strip())
    h.home = home
    assert h.wait_up(30), f"{name} did not start"
    return h


def stop(name):
    p = procs.pop(name, None)
    if p:
        p.terminate()
        try:
            p.wait(20)
        except subprocess.TimeoutExpired:
            p.kill()


def rpc(hub, m, p=None, wait=20):
    v = View(hub)
    try:
        v.next += 1
        i = v.next
        v.send({"id": i, "m": m, "p": p or {}})
        r = v.until(lambda j: str(j.get("id")) == str(i))
        return r["result"] if isinstance(r, dict) and isinstance(r.get("result"), dict) else r
    finally:
        v.close()


def events(hub):
    out = []
    for line in open(os.path.join(hub.home, "events.jsonl"), errors="replace"):
        try:
            out.append(json.loads(line))
        except ValueError:
            pass
    return out


def setting(hub, key):
    v = ""
    for e in events(hub):
        if e.get("$") == "SettingSet" and e.get("key") == key:
            v = e.get("value", "")
    return v


def uber(hub):
    u = ""
    for e in events(hub):
        if e.get("$") == "UberSet":
            u = e.get("bot", "")
    return u


def live_bots(hub):
    b = {}
    for e in events(hub):
        if e.get("$") == "BotSet":
            b[e["id"]] = e
        elif e.get("$") == "BotDeleted":
            b.pop(e.get("id"), None)
    return b


def wait(f, seconds=60):
    end = time.time() + seconds
    while time.time() < end:
        try:
            v = f()
            if v:
                return v
        except Exception:
            pass
        time.sleep(1)
    return None


def post_hook(base, hid, token, body=b'{"x":1}'):
    req = urllib.request.Request(f"{base}/hook/{hid}", data=body, method="POST", headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status
    except urllib.error.HTTPError as e:
        return e.code
    except OSError:
        return 0


def main():
    A, B = start("A"), start("B")
    # link: B makes a code, A asks with it, B approves
    code = rpc(B, "machines.code")
    pair = rpc(A, "machines.pair", {"address": B.base, "code": code.get("code", "")})
    req = wait(lambda: (rpc(B, "machines.requests") or {}).get("requests"))
    check("B sees A's request", bool(req), json.dumps(req)[:200])
    rpc(B, "machines.approve", {"request": req[0]["id"]})
    link_ab = wait(lambda: next((l for l, n in [(e["id"], e.get("name")) for e in events(A) if e.get("$") == "PeerSet" and not e.get("revoked")]), None))
    check("A and B linked", bool(link_ab), str(pair)[:160])

    # the home on A
    bub = rpc(A, "bots.create", {"name": "Bub"}).get("bot")
    cal = rpc(A, "bots.create", {"name": "Calendar"})
    rpc(A, "bots.uber", {"bot": bub})
    check("A's uberbot is Bub", wait(lambda: uber(A) == bub, 20), bub or "")
    rpc(A, "bots.room", {"name": "staff", "members": "Bub,Calendar"})
    rpc(A, "bots.remember", {"bot": bub, "key": "home", "text": "the desktop"})
    hook = rpc(A, "bots.hook", {"bot": bub, "name": "Pebble Index"})
    hid = hook.get("hook", "")
    tok = rpc(A, "bots.hook_token", {"hook": hid}).get("token", "")
    check("A has a webhook with a token", bool(hid and tok))
    # a phone: an invitation of A, enrolled: its own device token; it registers for push
    st, inv = A.invite()
    st2, enr = A.enroll(secret_of(inv.get("link", "")), "Simulated iPhone")
    phone = enr.get("token", "")
    check("the phone paired with A", st2 == 200 and bool(phone), str(st2))
    s = Socket(A, phone); hello = s.hello(); s.close()
    check("the phone's hello on A has no offer yet", hello.get("hubOffer", None) == "", str(hello.get("hubOffer")))
    v = View(A); v.close()
    rpc(A, "device.register", {"token": "ab" * 32, "platform": "ios", "kind": "alert", "env": "sandbox", "bundle": "dev.backplane.works"})

    # B is the ring's front door, while A owns the hook
    fr = wait(lambda: (lambda r: r if r.get("base") else None)(rpc(A, "ring.front.use", {"to": "hub-b"})), 30) or {}  # (B is named once A's link table has it)
    check("B fronts A's webhook", fr.get("base", "").startswith("http"), json.dumps(fr)[:200])
    check("a delivery to B reaches A while A owns it", post_hook(B.base, hid, tok) == 200)

    # the move
    links_a = [e["id"] for e in events(A) if e.get("$") == "PeerSet" and not e.get("revoked")]
    r = rpc(A, "home.move", {"to": "hub-b"})
    check("home.move starts", r.get("started") is True, json.dumps(r)[:200])
    done = wait(lambda: (lambda s: s if s.get("state") in ("done", "failed") else None)(rpc(A, "home.status")), 180)
    check("the move is done", (done or {}).get("state") == "done", json.dumps(done))
    check("B's uberbot is Bub (same id)", uber(B) == bub)
    check("B has Bub, Calendar", {"Bub", "Calendar"} <= {b.get("name") for b in live_bots(B).values()})
    check("A has no bots and no uberbot", not live_bots(A) and uber(A) == "")
    check("B holds Bub's memory", any(e.get("$") == "MemorySaved" and e.get("bot") == bub for e in events(B)))
    check("B holds the phone's push registration", any(e.get("$") == "DeviceRegistered" and e.get("token") == "ab" * 32 for e in events(B)))
    s = Socket(A, phone); hello = s.hello(); s.close()
    offer = json.loads(hello.get("hubOffer") or "{}")
    check("the phone's hello on A offers B, same credential", offer.get("credential") == "same" and offer.get("url", "").endswith(str(PORTS["B"])), json.dumps(offer))
    s = Socket(B, phone); hb = s.hello(); s.close()
    check("the phone's own token opens B", hb.get("t") == "hello", str(hb)[:120])
    take = rpc(A, "hub.offer.take", {"id": offer.get("id", "")})
    check("a client without a device token gets a pairing link of B", "/pair" in take.get("link", ""), json.dumps(take)[:160])
    check("B's hook token is A's", open(os.path.join(B.home, "secrets/hooks", hid + ".token")).read().strip() == tok)
    stop("A")
    check("the webhook lands on B with A stopped", post_hook(B.base, hid, tok) == 200)
    A = start("A")
    wait(lambda: rpc(A, "home.status") and True, 10)

    # and back
    r = rpc(A, "home.move", {"to": "back"})
    check("home.move back starts", r.get("started") is True, json.dumps(r)[:200])
    back = wait(lambda: uber(A) == bub and uber(B) == "", 180)
    check("Bub is A's uberbot again, B has none", bool(back))
    check("A has Bub and Calendar again", {"Bub", "Calendar"} <= {b.get("name") for b in live_bots(A).values()})
    n_mem = sum(1 for e in events(A) if e.get("$") == "MemorySaved" and e.get("bot") == bub)
    check("no line came back twice (one memory)", n_mem == 1, str(n_mem))
    check("a delivery to B reaches A again", wait(lambda: post_hook(B.base, hid, tok) == 200, 30))
    stop("A"); stop("B")
    finish()


if __name__ == "__main__":
    try:
        main()
    finally:
        for n in list(procs):
            stop(n)
