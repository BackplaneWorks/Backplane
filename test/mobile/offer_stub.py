#!/usr/bin/env python3
"""A stand-in hub for the home hub's offer (docs/pairing.md, Home hub): just enough data/1 for a phone to connect, list one project named after it,
and, playing the desktop, offer the other hub (hello `hubOffer`, feature hubOffer) and answer hub.offer.take / hub.offer.done.
usage: offer_stub.py PORT NAME TOKEN [OFFER_URL [same|take]]
  NAME      what its one project and thread are called (a phone's list shows which hub it is on)
  TOKEN     the device token it takes (the socket's ?token=, /client/capabilities' Bearer); /auth/enroll hands it out for any invite
  OFFER_URL the hub to offer (none: this is the home hub); `same` (default) offers credential "same", `take` leaves it out
GET /__seen lists the requests it took (method names, never a token). /hello needs no credential, as the hub's."""
import base64, hashlib, json, struct, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT, NAME, TOKEN = int(sys.argv[1]), sys.argv[2], sys.argv[3]
OFFER_URL = sys.argv[4] if len(sys.argv) > 4 else ""
CRED = sys.argv[5] if len(sys.argv) > 5 else "same"
OFFER = json.dumps({k: v for k, v in {"id": "offer-1", "name": "boat-hub", "url": OFFER_URL, "why": "home", "credential": "same" if CRED == "same" else None}.items() if v is not None}) if OFFER_URL else ""
SEEN = []
NOW = int(time.time() * 1000)
PROJECT = {"id": "p1", "title": NAME, "root": "/" + NAME, "origin": "", "threadCount": 1, "updatedAt": NOW, "waiting": 0, "icon": "", "color": "", "css": ""}
THREAD = {"id": "t1", "project": "p1", "title": "on " + NAME, "state": "idle", "updatedAt": NOW, "pinned": False, "pinOrder": 0, "settled": False, "settledAt": 0,
          "archived": False, "provider": "claude", "model": "", "preview": "", "waiting": 0, "parent": ""}


def reply(m, p):
    if m == "projects.list": return {"projects": [PROJECT], "next": None}
    if m == "threads.list": return {"threads": [THREAD], "next": None}
    if m == "thread.entries": return {"thread": THREAD, "entries": [], "next": None}
    if m == "subscribe": return {"sub": 1}
    if m == "hub.offer.take": return {"link": OFFER_URL + "/pair#invite=" + "ab" * 16}
    return {}


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def log_message(self, *a): pass
    def send(self, code, body):
        b = json.dumps(body).encode()
        self.send_response(code); self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_GET(self):
        p = self.path.split("?")[0]
        if p == "/__seen": return self.send(200, SEEN)
        if p == "/hello": return self.send(200, {"backplane": "stub", "name": NAME})
        if p == "/client/capabilities":
            SEEN.append({"http": p, "ok": self.headers.get("Authorization") == "Bearer " + TOKEN})
            return self.send(200, {"data": {"proto": "data/1"}}) if self.headers.get("Authorization") == "Bearer " + TOKEN else self.send(401, {"error": "pairing_required"})
        if p == "/ws": return self.socket()
        self.send(404, {})
    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length") or 0))
        if self.path == "/auth/enroll":
            SEEN.append({"http": "/auth/enroll", "name": self.headers.get("X-Backplane-Name", "")})
            return self.send(200, {"token": TOKEN})
        self.send(404, {})
    def socket(self):
        if "token=" + TOKEN not in self.path: return self.send(401, {"error": "pairing_required"})
        key = base64.b64encode(hashlib.sha1((self.headers["Sec-WebSocket-Key"] + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        self.send_response(101); self.send_header("Upgrade", "websocket"); self.send_header("Connection", "Upgrade"); self.send_header("Sec-WebSocket-Accept", key); self.end_headers(); self.wfile.flush()
        def frame(o):
            b = json.dumps(o).encode(); n = len(b)
            head = bytes([0x81]) + (bytes([n]) if n < 126 else bytes([126]) + struct.pack(">H", n) if n < 65536 else bytes([127]) + struct.pack(">Q", n))
            self.wfile.write(head + b); self.wfile.flush()
        SEEN.append({"ws": "hello"})
        frame({"t": "hello", "proto": "data/1", "hub": NAME, "name": NAME, "version": "stub", "epoch": "1", "now": int(time.time() * 1000), "features": ["hubOffer"], "hubOffer": OFFER})
        self.close_connection = True
        try:
            while True:
                h = self.rfile.read(2)
                if len(h) < 2 or h[0] & 15 == 8: return
                n = h[1] & 127
                if n == 126: n = struct.unpack(">H", self.rfile.read(2))[0]
                elif n == 127: n = struct.unpack(">Q", self.rfile.read(8))[0]
                mask = self.rfile.read(4); d = bytes(c ^ mask[i % 4] for i, c in enumerate(self.rfile.read(n)))
                if h[0] & 15 == 9: self.wfile.write(bytes([0x8A, 0])); self.wfile.flush(); continue
                if h[0] & 15 != 1: continue
                m = json.loads(d)
                SEEN.append({"m": m.get("m"), "p": {k: v for k, v in (m.get("p") or {}).items() if k != "token"}})
                frame({"t": "reply", "id": m["id"], "ok": True, "result": reply(m.get("m"), m.get("p") or {})})
        except (OSError, ValueError): return


ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
