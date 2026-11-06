#!/usr/bin/env python3
"""A stub of the hub's rescue routes for the phones (docs/PHONE-DATA.md, "The Hub screen"): /healthz, /hub/status, /hub/control, and just enough data/1 to say hello.
usage: hub_stub.py PORT [TOKEN]    POST /__mode {"mode": "...", "notice": "..."} changes what it plays; GET /__seen lists the requests it took.
Modes: up (a restart answers ok for 3 s, as the old hub does, then starting for 4 s, then ok), rescue, noguard, limit, redirect, refused (the socket gets a 401), hung (no answer)."""
import base64, hashlib, json, socket, struct, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(sys.argv[1]); TOKEN = sys.argv[2] if len(sys.argv) > 2 else "0123456789abcdef0123456789abcdef"
S = {"mode": "up", "notice": "", "restarted": 0.0}; SEEN = []
INCIDENTS = [{"at": 1793980000, "what": "Hub failed", "why": "exit 101", "did": "started again"}, {"at": 1793983600, "what": "Rolled back", "why": "asked from remote", "did": "abc marked bad, now on old"}]
ACTIONS = [{"id": "restart", "label": "Restart hub"}, {"id": "safe", "label": "Restart in safe mode", "text": "Agents, routines and bots stay paused until the next restart."}, {"id": "rollback", "label": "Roll back to old", "text": "Marks abc bad.", "remote": False}]

def health():
    if S["mode"] == "rescue": return 503, {"status": "down", "rescue": 1, "control": 1, "why": "exit 101"}
    age = time.time() - S["restarted"]
    if 3 <= age < 7: return 503, {"status": "starting", "phase": "fold", "safe": False}
    return 200, {"status": "ok", "safe": False, "control": 1, "guarded": True}
def notice(): return {"text": S["notice"], "tone": "warn", "actions": ACTIONS}
def status():
    if S["mode"] == "rescue": return {"guard": {"state": "rescue", "mode": "normal", "build": "abc", "text": "Hub is down: exit 101", "actions": ACTIONS[:1]}, "incidents": INCIDENTS, "rescue": 1}
    if S["mode"] == "noguard": return {"guard": None, "incidents": [], "hub": health()[1], "notice": {"text": S["notice"], "tone": "info", "actions": None}}
    return {"guard": {"state": "running", "mode": "normal", "build": "abc", "text": "Running abc", "actions": ACTIONS}, "incidents": INCIDENTS, "hub": health()[1], "notice": notice()}

class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def log_message(self, *a): pass
    def send(self, code, body, extra=()):
        b = json.dumps(body).encode() if body is not None else b""
        self.send_response(code); self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(b)))
        for k, v in extra: self.send_header(k, v)
        self.end_headers(); self.wfile.write(b)
    def allowed(self): return self.headers.get("Authorization") == "Bearer " + TOKEN
    def body(self): return self.rfile.read(int(self.headers.get("Content-Length") or 0)).decode()
    def note(self, body=""): SEEN.append({"m": self.command, "path": self.path, "auth": self.headers.get("Authorization"), "body": body, "t": time.time()})
    def do_GET(self):
        p = self.path.split("?")[0]
        if p == "/__seen": return self.send(200, SEEN)
        if p == "/ws": return self.socket()
        self.note()
        if S["mode"] == "hung": time.sleep(30); self.close_connection = True; return
        if p == "/healthz": c, b = health(); return self.send(c, b)
        if p == "/hub/status":
            if not self.allowed(): return self.send(401, {"ok": False, "text": "this Backplane needs its pairing token"})
            return self.send(200, status())
        self.send(404, None)
    def do_POST(self):
        b = self.body()
        if self.path == "/__mode": S.update(json.loads(b)); S["restarted"] = 0.0; SEEN.clear(); return self.send(200, S)
        self.note(b)
        if S["mode"] == "hung": time.sleep(30); self.close_connection = True; return
        if self.path != "/hub/control": return self.send(404, None)
        if S["mode"] == "redirect": return self.send(302, None, [("Location", "/elsewhere")])
        if not self.allowed(): return self.send(401, {"ok": False, "text": "this Backplane needs its pairing token"})
        if S["mode"] == "noguard": return self.send(409, {"ok": False, "text": "this hub runs without a guard: restart it at its machine"})
        if S["mode"] == "limit": return self.send(409, {"ok": False, "text": "three restarts in five minutes from other devices: wait, or use `backplane hub` at the machine"})
        action = json.loads(b or "{}").get("action")
        if action == "rollback": return self.send(409, {"ok": False, "text": "old has no remote control: going back to it from here would lock you out"})
        if S["mode"] == "rescue": S["mode"] = "up"
        S["restarted"] = time.time()
        self.send(202, {"ok": True, "text": "restarting"})
    # just enough of data/1: hello with the notice, an empty reply to every request (text frames, unfragmented)
    def socket(self):
        if S["mode"] == "refused": return self.send(401, {"ok": False, "text": "this Backplane needs its pairing token"})
        if S["mode"] in ("rescue", "hung") or TOKEN not in self.path: return self.send(503, {"status": "down", "rescue": 1})
        key = base64.b64encode(hashlib.sha1((self.headers["Sec-WebSocket-Key"] + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        self.send_response(101); self.send_header("Upgrade", "websocket"); self.send_header("Connection", "Upgrade"); self.send_header("Sec-WebSocket-Accept", key); self.end_headers(); self.wfile.flush()
        def frame(o):
            b = json.dumps(o).encode(); n = len(b)
            self.wfile.write(bytes([0x81]) + (bytes([n]) if n < 126 else bytes([126]) + struct.pack(">H", n)) + b); self.wfile.flush()
        frame({"t": "hello", "proto": "data/1", "hub": "stub", "version": "stub", "epoch": "1", "now": int(time.time() * 1000), "features": [], "notice": notice()})
        self.close_connection = True
        try:
            while True:
                h = self.rfile.read(2)
                if len(h) < 2 or h[0] & 15 == 8: return
                n = h[1] & 127
                if n == 126: n = struct.unpack(">H", self.rfile.read(2))[0]
                elif n == 127: n = struct.unpack(">Q", self.rfile.read(8))[0]
                mask = self.rfile.read(4); d = bytes(c ^ mask[i % 4] for i, c in enumerate(self.rfile.read(n)))
                if h[0] & 15 == 1:
                    m = json.loads(d)
                    frame({"t": "reply", "id": m["id"], "ok": True, "result": {"sub": 1, "projects": [], "threads": [], "next": None}})
        except (OSError, ValueError): return

ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
