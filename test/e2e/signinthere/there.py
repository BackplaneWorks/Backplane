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
PORTS = {"A": 3951, "B": 3952}
HERE = os.path.dirname(os.path.abspath(__file__))
US = "\x1f"
procs = {}


KEY_MUSE = "xai-" + "Q" * 40          # typed with "This is a key": kept in B's vault, typed from it
KEY_OPEN = "sk-ant-api03-" + "Z" * 40  # typed as a plain line: the screen echoes it, the panel must not


def agents(name):
    d = os.path.join(WORK, "agents-" + name); os.makedirs(d, exist_ok=True)
    for n in ("claude", "codex", "agy") + (("muse", "opencode") if name == "B" else ()):
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


class Listen:
    """A client of a hub, held open in the background: every frame it is sent (info, terminal bytes, views), kept as text to search."""
    def __init__(self, hub):
        import threading
        self.v, self.seen, self.on = View(hub), [], True
        self.t = threading.Thread(target=self.run, daemon=True); self.t.start()
    def run(self):
        from settings_revoke import cbor  # noqa
        while self.on:
            try:
                op, body = self.v.frame()
                if op == 2: self.seen.append(json.dumps(cbor(body)[0], default=str))
                elif op == 1: self.seen.append(body.decode(errors="replace"))
            except Exception:
                if not self.on: return
    def text(self): return "\n".join(self.seen)
    def close(self):
        self.on = False
        try: self.v.close()
        except Exception: pass


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
    m = row(A, "Oh My Pi")
    check("Oh My Pi has no installer: a terminal there", ("machine-signin-shell", link + US + "omp") in buttons(m), json.dumps(m))

    # Codex: device code, shown here; a phone's poll of the page all along (every 0.3 s), as the apps poll while busy
    import threading
    polls, polling = [], [True]
    def phone():
        while polling[0]:
            try:
                pg = rpc(A, "machines.page"); c = next((r for r in pg.get("rows") or [] if r.get("label", "").startswith("Codex")), {})
                polls.append((time.time(), pg.get("busy"), c.get("label", ""), c.get("note", "")))
            except Exception: pass
            time.sleep(0.3)
    pt = threading.Thread(target=phone, daemon=True); pt.start()
    act(A, "machine-signin", link + US + "codex")
    cx = wait(lambda: (lambda r: r if r and "QWER-TY123" in r.get("note", "") else None)(row(A, "Codex: signing in on hub-b")), 30)
    check("Codex's link and code, from B, show on A", bool(cx) and ("open-url", "https://auth.openai.com/codex/device") in buttons(cx) and ("copy", "QWER-TY123") in buttons(cx), json.dumps(cx)[:500])
    open(os.path.join(B.user, ".standin-approve"), "w").write("1")  # the person approves in the browser
    done = wait(lambda: (lambda r: r if r and r.get("tone") == "ok" else None)(row(A, "Codex")), 40)
    check("then Codex is signed in on B (A asked again by itself)", bool(done) and "Signed in on hub-b" in done.get("note", ""), json.dumps(row(A, "Codex")))
    time.sleep(1.5); polling[0] = False; pt.join(10)
    started = next((i for i, p in enumerate(polls) if p[1] is True), None)
    stale = [p for p in polls[started or 0:] if p[1] is False and "not signed in" in p[3]] if started is not None else ["never busy"]
    check("no page says not busy with the status from before the run (a phone stops reading there)", started is not None and not stale, str(stale)[:300])
    check("the last page a phone read says signed in", bool(polls) and polls[-1][1] is False and "Signed in on hub-b" in polls[-1][3], str(polls[-3:])[:300])
    check("the login is B's own, nothing on A", os.path.exists(os.path.join(B.user, ".standin-codex")) and not os.path.exists(os.path.join(A.user, ".standin-codex")))

    # Claude: link here, the code typed here goes to B's terminal
    act(A, "machine-signin", link + US + "claude")
    cl = wait(lambda: row(A, "Claude: signing in on hub-b"), 30)
    url = next((v for a, v in buttons(cl) if a == "open-url"), "")
    check("Claude's whole link shows on A", url.startswith("https://claude.com/cai/oauth/authorize?") and url.endswith("state=LOGINSTATE"), url[:300])
    check("with a box to type in", any(f.get("name") == "sikeys" for f in (cl or {}).get("fields") or []), json.dumps(cl)[:300])
    f = next((x for x in (cl or {}).get("fields") or [] if x.get("name") == "sikeys"), {})
    check("the box is masked and the hub holds nothing of it", f.get("secret") is True and f.get("vault") is True and f.get("value") == "", json.dumps(f))
    act(A, "secret-save", "@signin" + US + link + US + "claude", sikeys="CODE#LOGINSTATE")
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

    la, lb = Listen(A), Listen(B)  # every frame A's and B's clients get from here on
    # a key typed with "This is a key": through B's vault; the CLI echoes it and shows an old token unasked
    act(A, "machine-signin", link + US + "muse")
    mu = wait(lambda: (lambda r: r if r and "Paste your API key" in r.get("note", "") else None)(row(A, "Muse: signing in on hub-b")), 30)
    check("Muse's key prompt shows on A, its old token hidden", bool(mu) and ("ghp_" + "A" * 30) not in json.dumps(mu) and "[hidden]" in mu.get("note", ""), json.dumps(mu)[:400])
    act(A, "machine-signin-keymode", link + US + "muse")
    km = row(A, "Muse: signing in on hub-b")
    check("This is a key is on, and its Send goes through the vault there", any(b.get("action") == "machine-signin-keymode" and b.get("on") for b in (km or {}).get("buttons") or []) and ("secret-save", "@signin-key" + US + link + US + "muse") in buttons(km), json.dumps(km)[:400])
    act(A, "secret-save", "@signin-key" + US + link + US + "muse", sikeys=KEY_MUSE)
    echoed = wait(lambda: (lambda r: r if r and "Signed in." in r.get("note", "") else None)(row(A, "Muse: signing in on hub-b")), 30)
    check("the key reached Muse on B, and its echo is hidden", bool(echoed) and KEY_MUSE not in json.dumps(echoed) and "Got key" in echoed.get("note", ""), json.dumps(echoed)[:400])
    act(A, "machine-signin-stop", link + US + "muse")
    # a key typed as a plain line: echoed, hidden all the same
    act(A, "machine-signin", link + US + "opencode")
    wait(lambda: (lambda r: r if r and "Enter API key" in r.get("note", "") else None)(row(A, "OpenCode: signing in on hub-b")), 30)
    act(A, "secret-save", "@signin" + US + link + US + "opencode", sikeys=KEY_OPEN)
    oc = wait(lambda: (lambda r: r if r and "Saved" in r.get("note", "") else None)(row(A, "OpenCode: signing in on hub-b")), 30)
    check("a plain line that is a key is hidden in its echo", bool(oc) and KEY_OPEN not in json.dumps(oc), json.dumps(oc)[:400])
    st = rpc(A, "machines.page")
    check("machines.page says busy while a sign-in runs there", st.get("busy") is True, json.dumps(st)[:200])
    act(A, "machine-signin-stop", link + US + "opencode")
    time.sleep(1); la.close(); lb.close()
    check("A's clients were sent the panel's info as it changed", "signin.there" in la.text() and "Got key" in la.text(), la.text()[-300:])
    for k in (KEY_MUSE, KEY_OPEN):
        check(f"{k[:6]}…: not in what B sent A (signin.state, the panel's info, the page)", k not in la.text() and k not in json.dumps(rpc(A, "machines.page")))
        check(f"{k[:6]}…: not in anything B's own clients were sent (its sign-in terminal is quiet)", k not in lb.text())
    leaks = []
    for root, _, files in os.walk(WORK):
        for fn in files:
            path = os.path.join(root, fn)
            try: data = open(path, "rb").read()
            except OSError: continue
            for k in (KEY_MUSE, KEY_OPEN):
                if k.encode() in data: leaks.append(os.path.relpath(path, WORK))
    check("neither key is in any file of either hub (logs, events, drafts, vault)", not leaks, str(leaks))

    # the Claude sync token, made here
    act(A, "machine-signin-sync")
    sy = wait(lambda: (lambda r: r if r and ("secret-save", "@signin-sync") in buttons(r) else None)(row(A, "Claude sync token")), 30)
    check("the sync token's sign-in shows its link and a code box on A", bool(sy) and any(a == "open-url" for a, _ in buttons(sy)), json.dumps(row(A, "Claude sync token"))[:400])
    act(A, "secret-save", "@signin-sync", sicode="CODE#SYNCSTATE")
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
