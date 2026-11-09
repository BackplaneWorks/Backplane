#!/usr/bin/env python3
"""A mocked Google for the phone checks (the routes of native/host/src/google/tests.rs `Mock`; every token is made up).
usage: mock.py PORT   ->  BACKPLANE_GOOGLE_AUTH=http://HOST:PORT/auth  BACKPLANE_GOOGLE_OAUTH=http://127.0.0.1:PORT  BACKPLANE_GOOGLE_API=http://127.0.0.1:PORT
/auth answers 302 to the redirect_uri with code=4%2F0-test-code&state=<state> (the browser lands on the phone's own listener)."""
import base64, json, os, sys, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
D = os.path.dirname(os.path.abspath(__file__))
def f(n): return open(os.path.join(D, n)).read()
def idt(email): return "h.%s.s" % base64.urlsafe_b64encode(json.dumps({"email": email}).encode()).decode().rstrip("=")
class H(BaseHTTPRequestHandler):
    def log_message(self, *a): sys.stderr.write("mock %s\n" % (a[0] % a[1:],)); sys.stderr.flush()
    def go(self, m):
        u = urllib.parse.urlparse(self.path); q = urllib.parse.parse_qs(u.query)
        n = int(self.headers.get("Content-Length") or 0); body = self.rfile.read(n).decode() if n else ""
        if m == "GET" and u.path == "/auth":
            r = q.get("redirect_uri", [""])[0]; s = q.get("state", [""])[0]
            self.send_response(302); self.send_header("Location", r + "?" + urllib.parse.urlencode({"code": "4/0-test-code", "state": s, "scope": "email"}, quote_via=urllib.parse.quote)); self.send_header("Content-Length", "0"); self.end_headers(); return
        code, out = 404, '{"error":{"code":404,"message":"no such mock route"}}'
        if m == "POST" and u.path == "/token":
            b = urllib.parse.parse_qs(body)
            if b.get("client_id", [""])[0] != "test-client.apps.googleusercontent.com" or b.get("client_secret", [""])[0] != "test-client-secret": code, out = 401, '{"error":"invalid_client"}'
            elif b.get("grant_type", [""])[0] == "authorization_code":
                if b.get("code", [""])[0] != "4/0-test-code": code, out = 400, '{"error":"invalid_grant"}'
                else: code, out = 200, json.dumps({"access_token": "ya29.test-access-1", "expires_in": 3599, "refresh_token": "1//test-refresh-not-a-real-token", "scope": "openid https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/userinfo.email", "token_type": "Bearer", "id_token": idt("bub@example.org")})
            else: code, out = 200, json.dumps({"access_token": "ya29.test-access-2", "expires_in": 3599, "token_type": "Bearer"})
        elif m == "POST" and u.path == "/revoke": code, out = 200, "{}"
        elif m == "GET" and u.path == "/gmail/v1/users/me/messages": code, out = 200, f("message_list.json")
        elif m == "GET" and u.path.startswith("/gmail/v1/users/me/messages/"): code, out = 200, f("message_full.json" if "format=full" in u.query else "message_meta.json")
        elif m == "GET" and u.path == "/calendar/v3/calendars/primary/events": code, out = 200, f("events.json")
        self.send_response(code); self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(out.encode()))); self.end_headers(); self.wfile.write(out.encode())
    def do_GET(self): self.go("GET")
    def do_POST(self): self.go("POST")
ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
