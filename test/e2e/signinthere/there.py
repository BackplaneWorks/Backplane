#!/usr/bin/env python3
"""Sign in there end to end (docs/ALWAYS-ON.md, section 3) on two temp-home hubs: A plays the desktop, B the always-on boat-hub, linked by an approved
pairing. Everything goes through A's Settings, Machines (`machines.page` and `machines.page.act`, what the desktop's presenter and the phones send):
open B's panel; Codex's device sign-in on B shows its link and code here and ends signed in; Claude's sign-in on B shows its link, the code typed here
reaches it there; agy's terminal-only sign-in is driven by keys from here; a CLI B lacks says Not installed; the Claude sync token is made on A and
said to reach B. No provider is contacted (standin.py). Exit 1 when a check failed. Python 3.9, standard library only.

  there.py HUB_BIN WORK
"""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../pairing"))
from proto import Hub, check, finish  # noqa: E402
from settings_revoke import View  # noqa: E402

BIN, WORK = sys.argv[1], sys.argv[2]
PORTS = {"A": 3911, "B": 3912}
HERE = os.path.dirname(os.path.abspath(__file__))
US = "\x1f"
procs = {}


def agents(name):
    d = os.path.join(WORK, "agents-" + name); os.makedirs(d, exist_ok=True)
    for n in ("claude", "codex", "agy"):
        with open(os.path.join(d, n), "w") as f: f.write(f'#!/bin/sh\nexec python3 -I {HERE}/standin.py {n} "$@"\n')
        os.chmod(os.path.join(d, n), 0o755)
    return d


def start(name):
    home = os.path.join(WORK, name); os.makedirs(home, exist_ok=True)
    ev = os.path.join(home, "events.jsonl")
    if not os.path.exists(ev):
        os.makedirs(os.path.join(WORK, "proj-" + name), exist_ok=True)
        with open(ev, "w") as f:
            f.write(json.dumps({"$": "ProjectCreated", "id": "p1", "title": "there-" + name, "root": os.path.join(WORK, "proj-" + name), "at": 1}) + "\n")
            for k, v in (("onboard.done", "1"), ("onboard.uber", "1"), ("restart.continue", "off")):
                f.write(json.dumps({"$": "SettingSet", "key": k, "value": v, "at": int(time.time()) - 90}) + "\n")
    env = dict(os.environ, BACKPLANE_AGENTS=agents(name), BACKPLANE_NO_UPDATE="1", BACKPLANE_NAME=f"hub-{name.lower()}", HOME=os.path.join(WORK, f"home-{name}"),
               BACKPLANE_VAULT_KEYRING="file", STANDIN_EMAIL="boat@example.com")
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
    h = Hub(f"http://127.0.0.1:{PORTS[name]}", open(tok).read().strip()); h.home = home; h.user = env["HOME"]
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
        v.next += 1; i = v.next
        v.send({"id": i, "m": m, "p": p or {}})
        r = v.until(lambda j: str(j.get("id")) == str(i), )
        return r["result"] if isinstance(r, dict) and isinstance(r.get("result"), dict) else r
    finally:
        v.close()


def wait(f, seconds=60):
    end = time.time() + seconds
    while time.time() < end:
        try:
            v = f()
            if v: return v
        except Exception:
            pass
        time.sleep(0.5)
    return None


def rows(A): return (rpc(A, "machines.page") or {}).get("rows") or []
def row(A, label):
    return next((r for r in rows(A) if r.get("label") == label), None)
def act(A, action, value="", **fields): return rpc(A, "machines.page.act", {"action": action, "value": value, "fields": fields})
def buttons(r): return [(b.get("action"), b.get("value")) for b in (r or {}).get("buttons") or []]


def main():
    A, B = start("A"), start("B")
    code = rpc(B, "machines.code")
    rpc(A, "machines.pair", {"address": B.base, "code": code.get("code", "")})
    req = wait(lambda: (rpc(B, "machines.requests") or {}).get("requests"))
    check("B sees A's request", bool(req), json.dumps(req)[:200])
    if not req: return finish()
    rpc(B, "machines.approve", {"request": req[0]["id"]})
    b_row = wait(lambda: next((r for r in rows(A) if r.get("label") == "hub-b" and "Paired" in r.get("note", "")), None), 60)
    check("A lists B as paired, with Sign in there", bool(b_row) and any(a == "machine-signin-open" for a, _ in buttons(b_row)), json.dumps(b_row)[:300])
    if not b_row: return finish()
    link = next(v for a, v in buttons(b_row) if a == "machine-signin-open")

    # the panel: what B has, asked of B
    act(A, "machine-signin-open", link)
    head = wait(lambda: row(A, "Codex"), 60)
    check("B's panel opens on A", bool(row(A, "Sign in on hub-b")) and bool(head), json.dumps(rows(A))[:600])
    check("Codex on B: installed, not signed in", "not signed in" in (row(A, "Codex") or {}).get("note", "") and "0.167.0" in row(A, "Codex")["note"], json.dumps(row(A, "Codex")))
    check("Claude on B: not signed in", "not signed in" in (row(A, "Claude") or {}).get("note", ""), json.dumps(row(A, "Claude")))
    g = row(A, "Grok")
    check("Grok is not on B, with Install there", "Not installed on hub-b" in (g or {}).get("note", "") and ("machine-signin-install", link + US + "grok") in buttons(g), json.dumps(g))
    m = row(A, "Muse")
    check("Muse has no installer: a terminal there", ("machine-signin-shell", link + US + "muse") in buttons(m), json.dumps(m))

    # Codex: device code, shown here
    act(A, "machine-signin", link + US + "codex")
    cx = wait(lambda: (lambda r: r if r and "QWER-TY123" in r.get("note", "") else None)(row(A, "Codex: signing in on hub-b")), 30)
    check("Codex's link and code, from B, show on A", bool(cx) and ("open-url", "https://auth.openai.com/codex/device") in buttons(cx) and ("copy", "QWER-TY123") in buttons(cx), json.dumps(cx)[:500])
    open(os.path.join(B.user, ".standin-approve"), "w").write("1")  # the person approves in the browser
    done = wait(lambda: (lambda r: r if r and r.get("tone") == "ok" else None)(row(A, "Codex")), 40)
    check("then Codex is signed in on B (A asked again by itself)", bool(done) and "Signed in on hub-b" in done.get("note", ""), json.dumps(row(A, "Codex")))
    check("the login is B's own, nothing on A", os.path.exists(os.path.join(B.user, ".standin-codex")) and not os.path.exists(os.path.join(A.user, ".standin-codex")))

    # Claude: link here, the code typed here goes to B's terminal
    act(A, "machine-signin", link + US + "claude")
    cl = wait(lambda: row(A, "Claude: signing in on hub-b"), 30)
    url = next((v for a, v in buttons(cl) if a == "open-url"), "")
    check("Claude's whole link shows on A", url.startswith("https://claude.com/cai/oauth/authorize?") and url.endswith("state=LOGINSTATE"), url[:300])
    check("with a box to type in", any(f.get("name") == "sikeys" for f in (cl or {}).get("fields") or []), json.dumps(cl)[:300])
    act(A, "machine-signin-keys", link + US + "claude", sikeys="CODE#LOGINSTATE")
    ok = wait(lambda: (lambda r: r if r and r.get("tone") == "ok" else None)(row(A, "Claude")), 40)
    check("Claude is signed in on B, as who it says", bool(ok) and "as boat@example.com" in ok.get("note", ""), json.dumps(row(A, "Claude")))

    # agy: a terminal-only flow, driven by keys from here
    act(A, "machine-signin", link + US + "agy")
    ag = wait(lambda: (lambda r: r if r and "Press Enter" in r.get("note", "") else None)(row(A, "Antigravity: signing in on hub-b")), 30)
    check("agy's screen shows on A", bool(ag), json.dumps(row(A, "Antigravity: signing in on hub-b"))[:400])
    act(A, "machine-signin-key", link + US + "agy" + US + "enter")
    ag2 = wait(lambda: (lambda r: r if r and "Signed in as agy@example.com" in r.get("note", "") else None)(row(A, "Antigravity: signing in on hub-b")), 20)
    check("Enter from A reaches it on B", bool(ag2))
    act(A, "machine-signin-stop", link + US + "agy")
    st = wait(lambda: row(A, "Antigravity"), 20)
    check("Stop ends it there", bool(st) and "cannot ask it" in st.get("note", ""), json.dumps(st))

    # the Claude sync token, made here
    act(A, "machine-signin-sync")
    sy = wait(lambda: (lambda r: r if r and any(a == "machine-signin-sync-code" for a, _ in buttons(r)) else None)(row(A, "Claude sync token")), 30)
    check("the sync token's sign-in shows its link and a code box on A", bool(sy) and any(a == "open-url" for a, _ in buttons(sy)), json.dumps(row(A, "Claude sync token"))[:400])
    act(A, "machine-signin-sync-code", "", sicode="CODE#SYNCSTATE")
    reach = wait(lambda: (lambda r: r if r and "on hub-b" in r.get("note", "") else None)(row(A, "Claude sync token")), 60)
    check("the sync token is said to reach B", bool(reach), json.dumps(row(A, "Claude sync token")))
    check("and B holds it (the same token, never a login file)", not os.path.exists(os.path.join(B.user, ".claude")))

    # an away machine says so
    stop("B")
    act(A, "machine-signin-open", link)
    away = wait(lambda: row(A, "Could not reach it"), 30)
    check("with B down the panel says it cannot reach it", bool(away) and "away" in away.get("note", ""), json.dumps(away))
    act(A, "machine-signin-close", link)
    check("Close takes the panel away", wait(lambda: row(A, "Sign in on hub-b") is None, 10))
    stop("A")
    finish()


if __name__ == "__main__":
    try:
        main()
    finally:
        for n in list(procs): stop(n)
