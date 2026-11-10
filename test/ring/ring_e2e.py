#!/usr/bin/env python3
"""The Pebble Index ring end to end on a temp hub (docs/ring.md): stand-in agents, the real voice helper and local model, a real M4A.

    python3 -I test/ring/ring_e2e.py HUB_BIN VOICE_BIN LOCAL_DIR SAMPLE.m4a

HUB_BIN: a built backplane-hub; VOICE_BIN: a built backplane-voice; LOCAL_DIR: a voice/local with a downloaded model (only read);
SAMPLE: a recording as the Pebble app sends it (AAC in M4A, mono, 16 kHz). Exits 1 on any failure; the temp home is under $BP_TMP."""
import base64, json, os, socket, struct, subprocess, sys, tempfile, time, urllib.request

HUB, VOICE, LOCAL, SAMPLE = sys.argv[1:5]
fails = []
def check(ok, what, got=""):
    print(("PASS " if ok else "FAIL ") + what + ("" if ok else f"  ({got})"), flush=True)
    if not ok: fails.append(what)

class Ws:
    """Just enough WebSocket for the hub's JSON requests: masked text frames out, replies by id."""
    def __init__(self, port, token):
        self.s = socket.create_connection(("127.0.0.1", port), timeout=30); self.buf = b""; self.n = 100
        key = base64.b64encode(os.urandom(16)).decode()
        self.s.sendall(f"GET /ws?data=1&enc=json&token={token} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {key}\r\n\r\n".encode())
        while b"\r\n\r\n" not in self.buf: self.buf += self.s.recv(4096)
        head, _, self.buf = self.buf.partition(b"\r\n\r\n"); assert b" 101 " in head.split(b"\r\n")[0], head
    def need(self, n):
        while len(self.buf) < n:
            d = self.s.recv(65536)
            if not d: raise EOFError
            self.buf += d
        o, self.buf = self.buf[:n], self.buf[n:]; return o
    def frame(self):
        b0, b1 = self.need(2); n = b1 & 127
        if n == 126: n = struct.unpack(">H", self.need(2))[0]
        elif n == 127: n = struct.unpack(">Q", self.need(8))[0]
        return b0 & 15, self.need(n)
    def send(self, text):
        b = text.encode(); m = os.urandom(4); h = bytes([0x81]); n = len(b)
        h += bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n) if n < 65536 else bytes([0x80 | 127]) + struct.pack(">Q", n)
        self.s.sendall(h + m + bytes(c ^ m[i % 4] for i, c in enumerate(b)))
    def rpc(self, m, p):
        self.n += 1; rid = self.n; self.send(json.dumps({"id": rid, "m": m, "p": p}))
        end = time.time() + 30
        while time.time() < end:
            op, body = self.frame()
            if op != 1: continue
            try: j = json.loads(body)
            except ValueError: continue
            if j.get("t") == "reply" and j.get("id") == rid: return dict(j.get("result") or {}, ok=j.get("ok"), error=j.get("error"))
        raise TimeoutError(m)

def post(url, headers, body):
    r = urllib.request.Request(url, data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(r, timeout=30) as a: return a.status, a.read().decode()
    except urllib.error.HTTPError as e: return e.code, e.read().decode()

def form(fields, audio):
    b = b"--bnd\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"rec-e2e.m4a\"\r\nContent-Type: audio/mp4\r\n\r\n" + audio + b"\r\n"
    for k, v in fields: b += f"--bnd\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n".encode()
    return b + b"--bnd--\r\n"

def main():
    tmp = os.environ.get("BP_TMP") or os.path.expanduser("~/.cache/bp-tmp"); os.makedirs(tmp, exist_ok=True)
    root = tempfile.mkdtemp(prefix="bpt-ring-", dir=tmp); home = f"{root}/home"; agents = f"{root}/bin"
    os.makedirs(f"{home}/voice", exist_ok=True); os.makedirs(agents); os.symlink(os.path.abspath(LOCAL), f"{home}/voice/local")
    for n in ["claude", "codex", "grok", "muse", "agy", "gemini"]:  # stand-ins: a bot's turn fails at once; no real model runs
        p = f"{agents}/{n}"; open(p, "w").write("#!/bin/sh\nexit 1\n"); os.chmod(p, 0o755)
    now = int(time.time())
    ev = [{"$": "SettingSet", "key": k, "value": v, "at": now} for k, v in [("restart.continue", "off"), ("onboard.done", "1"), ("onboard.uber", "1"), ("voice.engine", "local"), ("voice.model", "parakeet-v2"), ("voice.e.observed>", "observed -> remarked")]]
    open(f"{home}/events.jsonl", "w").write("".join(json.dumps(e) + "\n" for e in ev))
    s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
    env = dict(os.environ, BACKPLANE_AGENTS=agents, BACKPLANE_NO_UPDATE="1", BACKPLANE_VOICE_BIN=os.path.abspath(VOICE), HOME=f"{root}/fakehome", LC_ALL="C")
    os.makedirs(f"{root}/fakehome", exist_ok=True)
    hub = subprocess.Popen([HUB, "--home", home, "--port", str(port), "--host", "127.0.0.1", "--no-tailscale", "--foreground"], stdout=open(f"{root}/hub.log", "wb"), stderr=subprocess.STDOUT, env=env)
    base = f"http://127.0.0.1:{port}"
    try:
        for _ in range(100):
            try: urllib.request.urlopen(base + "/healthz", timeout=2); break
            except Exception: time.sleep(0.2)
        tok = open(f"{home}/token").read().strip(); ws = Ws(port, tok)
        su = ws.rpc("ring.setup", {}); check(su.get("ok") and su.get("hook") and len(su.get("token", "")) == 32, "Settings, Ring, Set up: Pebble, its webhook and token", su)
        url = f"{base}/hook/{su['hook']}"; bearer = {"Authorization": f"Bearer {su['token']}"}
        ct = {"Content-Type": "multipart/form-data; boundary=bnd"}
        audio = open(SAMPLE, "rb").read(); said = (now - 3 * 3600) * 1000
        hs = dict(ct, **bearer, **{"X-Index-Webhook-Version": "1", "X-Index-Delivery": "e2e-delivery-1", "X-Index-Trigger": "single-click-hold", "X-Audio-Size": str(len(audio))})
        body = form([("transcription", "the apps own words"), ("recordedAt", said), ("client", "ring")], audio)
        st, txt = post(url, dict(ct, **{"Authorization": "Bearer wrong"}), body); check(st == 401, "a wrong token is refused", (st, txt))
        st, txt = post(url, hs, body); check((st, txt) == (200, "received"), "the recording is taken", (st, txt))
        st, txt = post(url, hs, body); check((st, txt) == (200, "already received"), "the app's retry is a 2xx and nothing more", (st, txt))
        note = None
        for _ in range(240):
            l = ws.rpc("ring.list", {})
            n = (l.get("notes") or [{}])[0]
            if n.get("state") in ("sent", "failed"): note = n; break
            time.sleep(0.5)
        check(note is not None and note["state"] == "sent", "heard and given to Pebble", note)
        if note:
            check(note["model"] == "parakeet-v2" and isinstance(note.get("conf"), int), "heard by the local model, with a confidence", note)
            w = note["words"]; check("portrait" in w.lower() and "remarked" in w and "observed" not in w, "the words, with the dictionary applied", w)
            full = ws.rpc("ring.get", {"note": note["id"]})["note"]; h = full["heard"][-1]
            check(len(h["segs"]) >= 1 and all(a < b for a, b, _, _ in h["segs"]), "segments with their times", h["segs"])
            check(full["ring_text"] == "the apps own words" and full["recorded_ms"] == said and full["audio"] == len(audio), "kept: the app's words, when it was said, the recording", full)
            check(open(f"{home}/ring/{note['id']}/audio.m4a", "rb").read() == audio, "the recording kept byte for byte")
            got = [e for e in map(json.loads, open(f"{home}/events.jsonl")) if e.get("$") == "MessagePosted" and e.get("role") == "user" and e.get("text", "").startswith("[voice note from your Pebble ring")]
            check(len(got) == 1, "Pebble heard it once", len(got))
            if got: t = got[0]["text"]; check("Hold & talk" in t and "came 3 h after it was said" in t and note["id"] in t, "with the gesture, the delay and the note's id", t[:400])
            play = note["play"].replace(note["play"].split("/ring/")[0], base)
            with urllib.request.urlopen(play, timeout=10) as a: check(a.read() == audio and a.headers["Content-Type"] == "audio/mp4", "Play serves the recording")
            try: urllib.request.urlopen(f"{base}/ring/audio/{note['id']}?k=00", timeout=10); check(False, "a wrong link key is refused")
            except urllib.error.HTTPError as e: check(e.code == 404, "a wrong link key is refused", e.code)
            check(len(ws.rpc("ring.search", {"q": "portrait phebe"})["notes"]) == 1 and not ws.rpc("ring.search", {"q": "kicad"})["notes"], "search by the words")
            ws.rpc("ring.search", {"q": ""})
            r = ws.rpc("ring.hear", {"note": note["id"], "engine": "local:parakeet-v2"}); check(r.get("ok"), "Hear again is taken", r)
            again = None
            for _ in range(120):
                n2 = ws.rpc("ring.list", {})["notes"][0]
                if n2.get("heard") == 2: again = n2; break
                time.sleep(0.5)
            check(again is not None and again["model"] == "parakeet-v2", "heard again, kept beside the first hearing", again)
        t = dict(ct, **bearer, **{"X-Index-Webhook-Version": "1", "X-Index-Delivery": "t-1", "X-Index-Trigger": "test-event", "X-Index-Test": "true"})
        st, txt = post(url, t, b"--bnd\r\nContent-Disposition: form-data; name=\"client\"\r\n\r\nring\r\n--bnd--\r\n"); check(st == 200 and "test received" in txt, "Send test event", (st, txt))
        check(ws.rpc("ring.list", {})["total"] == 1, "a test is not kept")
    finally:
        hub.terminate(); hub.wait(10)
    print(f"{'FAILED' if fails else 'OK'}: {len(fails)} failed; home {root}")
    if not fails: subprocess.run(["rm", "-rf", root])
    sys.exit(1 if fails else 0)

main()
