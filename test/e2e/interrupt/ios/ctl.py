# ctl.py HOST PORT RIGDIR: the UI test's hands on the Linux side. POST /cutA /healA /cutB /healB (relay cut files),
# /send?thread=ID&text=T (a data/1 message.send on hub B, on loopback), /helper?text=T (the Helper's running turn calls thread_send into tb),
# /log?w=A|B&s=SUBSTR (200 when the stand-in log of that hub holds a line with SUBSTR). Every call is appended to RIGDIR/ctl.log.
import http.server, json, os, subprocess, sys, time, urllib.parse, urllib.request
H, P, R = sys.argv[1], int(sys.argv[2]), sys.argv[3]
DAPI = os.path.join(os.path.dirname(os.path.abspath(__file__)), "dapi.ts")
def log(s):
    with open(R + "/ctl.log", "a") as f: f.write(f"{time.strftime('%H:%M:%S')}.{int(time.time()*1000)%1000:03d} {s}\n")
def dapi(m, p): return subprocess.run([os.environ.get("BUN", os.path.expanduser("~/.bun/bin/bun")), DAPI, "3982", R + "/homeB/token", m, json.dumps(p)], capture_output=True, text=True, timeout=20).stdout
def helper(text):
    urls = [l.strip() for l in open(R + "/mcp.urls") if "/mcp/th." in l]
    if not urls: return 500, "no Helper MCP url"
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "thread_send", "arguments": {"threadId": "tb", "text": text}}}).encode()
    rq = urllib.request.Request(urls[-1], data=body, headers={"Content-Type": "application/json", "Accept": "application/json, text/event-stream"})
    with urllib.request.urlopen(rq, timeout=15) as r: return 200, r.read().decode()
class Hd(http.server.BaseHTTPRequestHandler):
    def do_POST(self): self.go()
    def do_GET(self): self.go()
    def go(self):
        u = urllib.parse.urlparse(self.path); q = dict(urllib.parse.parse_qsl(u.query)); code, out = 200, "ok"
        try:
            if u.path in ("/cutA", "/cutB"): open(f"{R}/cut{u.path[-1]}", "w").close()
            elif u.path in ("/healA", "/healB"):
                try: os.remove(f"{R}/cut{u.path[-1]}")
                except FileNotFoundError: pass
            elif u.path == "/send": out = dapi("message.send", {"thread": q["thread"], "text": q["text"]})
            elif u.path == "/helper": code, out = helper(q["text"])
            elif u.path == "/log":
                f = f"{R}/standin-{q['w']}.log"; code = 200 if os.path.exists(f) and any(q["s"] in l for l in open(f, errors="replace")) else 404
            elif u.path == "/now":
                # the stand-in took a message holding s as an interrupt: its USER line's uuid has a NOW line
                f = f"{R}/standin-{q['w']}.log"; L = open(f, errors="replace").read().splitlines() if os.path.exists(f) else []
                ids = [l.split()[1] for l in L if l.startswith("USER ") and q["s"] in l]
                hit = [l for l in L if l.startswith("NOW ") and l.split()[1] in ids]
                code, out = (200, hit[0]) if hit else (404, "no")
                if hit: log(f"/now {q} -> {[l for l in L if l.startswith('USER ') and q['s'] in l]} {hit}")
            elif u.path == "/mcp": code = 200 if os.path.exists(R + "/mcp.urls") and any(f"/mcp/{q['t']}." in l for l in open(R + "/mcp.urls")) else 404
            else: code, out = 404, "?"
        except Exception as e: code, out = 500, repr(e)
        if u.path not in ("/log", "/now", "/mcp"): log(f"{u.path} {q} -> {code} {out[:300]}")
        b = out.encode(); self.send_response(code); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer((H, P), Hd).serve_forever()
