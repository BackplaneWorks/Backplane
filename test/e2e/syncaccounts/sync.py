#!/usr/bin/env python3
"""Sync accounts end to end (docs/multiple-provider-accounts.md, "Sync accounts") on two temp-home hubs. Nothing of the person's is touched; no
provider is contacted: `claude` is claude_standin.py, which plays `setup-token` and writes down the token each child was started with.

  sync.py HUB_BIN WORK

A plays the desktop, B the always-on boat-hub, linked by an approved pairing (machines.code on B, machines.pair from A, machines.approve on B).
On A: a sync account is made (Add account, Claude (sync)): setup-token's link is shown, the pasted code goes to it, its token lands in A's vault.
Then: it appears on B with the same id; a turn on B (a thread whose new-thread default is that account) runs with the token in its environment; a
renewal on A reaches B; its removal on A removes it on B. Exit 1 when a check failed. Python 3.9, standard library only.
"""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../pairing"))
from proto import Hub, check, finish  # noqa: E402
from settings_revoke import View  # noqa: E402

BIN, WORK = sys.argv[1], sys.argv[2]
PORTS = {"A": 3871, "B": 3872}
HERE = os.path.dirname(os.path.abspath(__file__))
SEEN = os.path.join(WORK, "seen.txt")
TOKEN1, TOKEN2 = "sk-ant-oat01-" + "S" * 90, "sk-ant-oat01-" + "R" * 90
procs = {}


def agents():
    d = os.path.join(WORK, "agents"); os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "claude"), "w") as f: f.write(f'#!/bin/sh\nexec python3 -I {HERE}/claude_standin.py "$@"\n')
    for n in ("codex", "grok", "muse", "opencode"):
        with open(os.path.join(d, n), "w") as f: f.write("#!/bin/sh\nexit 1\n")
    for n in os.listdir(d): os.chmod(os.path.join(d, n), 0o755)
    return d


def start(name, token=TOKEN1):
    home = os.path.join(WORK, name); os.makedirs(home, exist_ok=True)
    ev = os.path.join(home, "events.jsonl")
    if not os.path.exists(ev):
        os.makedirs(os.path.join(WORK, "proj-" + name), exist_ok=True)
        with open(ev, "w") as f:
            f.write(json.dumps({"$": "ProjectCreated", "id": "p1", "title": "sync-" + name, "root": os.path.join(WORK, "proj-" + name), "at": 1}) + "\n")
            for k, v in (("onboard.done", "1"), ("onboard.uber", "1"), ("restart.continue", "off")):
                f.write(json.dumps({"$": "SettingSet", "key": k, "value": v, "at": int(time.time()) - 90}) + "\n")
    env = dict(os.environ, BACKPLANE_AGENTS=agents(), BACKPLANE_NO_UPDATE="1", BACKPLANE_NAME=f"hub-{name.lower()}", HOME=os.path.join(WORK, f"home-{name}"),
               BACKPLANE_VAULT_KEYRING="file", SYNC_SEEN=SEEN, STANDIN_TOKEN=token)
    for k in ("DISPLAY", "WAYLAND_DISPLAY", "BACKPLANE_URL", "BACKPLANE_HOOK_URL", "BACKPLANE_HTTPS_PORT", "CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY", "CLAUDE_CONFIG_DIR"):
        env.pop(k, None)
    os.makedirs(env["HOME"], exist_ok=True)
    log = open(os.path.join(WORK, f"{name}.log"), "a")
    procs[name] = subprocess.Popen([BIN, "--home", home, "--port", str(PORTS[name]), "--host", "127.0.0.1", "--no-tailscale", "--foreground", "--headless"],
                                   env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    tok = os.path.join(home, "token")
    for _ in range(100):
        if os.path.exists(tok): break
        time.sleep(0.1)
    h = Hub(f"http://127.0.0.1:{PORTS[name]}", open(tok).read().strip()); h.home = home
    assert h.wait_up(30), f"{name} did not start"
    return h


def stop(name):
    p = procs.pop(name, None)
    if p:
        p.terminate()
        try: p.wait(20)
        except subprocess.TimeoutExpired: p.kill()


def rpc(hub, m, p=None):
    v = View(hub)
    try:
        if m.startswith("accounts"):  # account requests go on a view session that offers the accounts wire version (accounts_hub.rs note_request)
            v.request("view.open", {"now": int(time.time() * 1000), "accountProfiles": 1})
        v.next += 1; i = v.next
        v.send({"id": i, "m": m, "p": p or {}})
        r = v.until(lambda j: str(j.get("id")) == str(i))
        return r["result"] if isinstance(r, dict) and isinstance(r.get("result"), dict) else r
    finally:
        v.close()


def acc(hub, **p): return rpc(hub, "accounts", dict(p, accountProfiles=1))


def wait(f, seconds=60):
    end = time.time() + seconds
    while time.time() < end:
        try:
            v = f()
            if v: return v
        except Exception:
            pass
        time.sleep(1)
    return None


def prof(hub, pid):
    return next((p for p in (acc(hub, op="list").get("profiles") or []) if p.get("id") == pid), None)


def seen():
    try: return [l.split() for l in open(SEEN)]
    except OSError: return []


def make_token(A, pid):
    att = wait(lambda: next((a for a in (acc(A, op="list").get("attempts") or []) if a.get("profile") == pid and a.get("state") == "waiting"), None), 60)
    check("setup-token's link is shown", bool(att) and att.get("url", "").startswith("https://claude.com/cai/oauth/authorize"), json.dumps(att)[:200])
    if not att: return False
    r = acc(A, op="respond", profile=pid, attempt=att["id"], code="CODE#SYNCSTATE")
    check("the pasted code is taken", not r.get("error"), json.dumps(r)[:200])
    return wait(lambda: (att["id"] in [a.get("id") for a in (acc(A, op="list").get("attempts") or []) if a.get("state") == "done"]), 60)


def main():
    A, B = start("A"), start("B")
    code = rpc(B, "machines.code")
    rpc(A, "machines.pair", {"address": B.base, "code": code.get("code", "")})
    req = wait(lambda: (rpc(B, "machines.requests") or {}).get("requests"))
    check("B sees A's request", bool(req), json.dumps(req)[:200])
    if not req: return finish()
    rpc(B, "machines.approve", {"request": req[0]["id"]})
    linked = wait(lambda: any(e.get("$") == "PeerSet" and not e.get("revoked") for e in map(json.loads, open(os.path.join(A.home, "events.jsonl")))), 60)
    check("A and B linked", bool(linked))

    # on A: Add account -> Claude (sync)
    s = acc(A, op="add", provider="claude", kind="sync", label="Max sync", start=True)
    pid = s.get("made", "")
    check("a sync account is added on A", bool(pid) and not s.get("error"), json.dumps(s)[:300])
    check("its token is made and checked", bool(make_token(A, pid)))
    pa = wait(lambda: (lambda p: p if p and p.get("state") == "connected" else None)(prof(A, pid)), 60)
    check("A: connected, a sync token for a year", bool(pa) and pa.get("kind") == "sync" and pa.get("tokenExpiresAt", 0) > time.time() + 360 * 86400, json.dumps(pa)[:300])
    snap = json.dumps(acc(A, op="list"))
    check("no client is told the token", TOKEN1 not in snap and "sk-ant-oat01" not in snap)
    for n in ("A",):
        logtxt = open(os.path.join(WORK, f"{n}.log"), errors="replace").read(); ev = open(os.path.join(A.home, "events.jsonl"), errors="replace").read()
        check("no log or timeline line has it", TOKEN1 not in logtxt and TOKEN1 not in ev)
    check("A's vault holds it, sealed", os.path.exists(os.path.join(A.home, "secrets", "vault.json")) and TOKEN1 not in open(os.path.join(A.home, "secrets", "vault.json")).read())

    # on B: the same account, and a turn on it gets the token
    pb = wait(lambda: prof(B, pid), 60)
    check("B has the account with the same id, from A", bool(pb) and pb.get("kind") == "sync" and pb.get("origin"), json.dumps(pb)[:300])
    if not pb: return finish()
    st = acc(B, op="status", profile=pid, asked=True)
    pb = prof(B, pid)
    check("B checks it on its own vault's token", (pb or {}).get("state") == "connected", json.dumps(pb)[:300])
    # (the default is checked against the inventory in the log, which follows the live accounts within a second)
    d = wait(lambda: (lambda r: r if isinstance(r, dict) and r.get("ok", True) is not False else None)(rpc(B, "accounts.default", {"provider": "claude", "profile": pid, "epoch": pb.get("epoch"), "accountProfiles": 1})), 30)
    check("it is B's default for new threads", bool(d), json.dumps(d)[:200])
    th = rpc(B, "thread.create", {"project": "p1", "title": "sync turn", "env": "local"})
    tid = (th or {}).get("thread", "")
    check("a thread on B", bool(tid), json.dumps(th)[:200])
    rpc(B, "turn.start", {"thread": tid, "text": "hello from the boat", "msg": "m-sync-1"})
    turn = wait(lambda: [l for l in seen() if l[1] == "TURN"], 90)
    check("the turn on B ran with the sync token", bool(turn) and turn[-1][3] == TOKEN1, str(turn)[:300])

    # renewal on A reaches B
    stop("A"); A = start("A", token=TOKEN2)
    acc(A, op="login", profile=pid)
    check("a renewal is made on A", bool(make_token(A, pid)))
    n0 = len(seen())
    def renewed():
        acc(B, op="status", profile=pid, asked=True)
        return any(l[3] == TOKEN2 for l in seen()[n0:] if len(l) > 3)
    check("the renewed token reaches B (its check runs on it)", bool(wait(renewed, 60)))
    check("and the same account and epoch there", (prof(B, pid) or {}).get("epoch") == pb.get("epoch"))

    # removal on A removes it on B
    r = acc(A, op="remove", profile=pid)
    check("removed on A", not r.get("error") and prof(A, pid) is None, json.dumps(r)[:200])
    check("and gone from B", bool(wait(lambda: prof(B, pid) is None, 60)))
    stop("A"); stop("B")
    finish()


if __name__ == "__main__":
    try:
        main()
    finally:
        for n in list(procs): stop(n)
