#!/usr/bin/env python3
# A page for the Browser tab's live check: a big button, a text box and a tall body. What the page sees is written to the file named by argv[2],
# one line each: "click <t>", "typed <the box's value>", "key <Enter|Tab|...>", "scrolled <scrollY>".
# usage: live_page.py PORT EVENTS_FILE
import http.server, sys, time, urllib.parse
PORT, EVENTS = int(sys.argv[1]), sys.argv[2]
PAGE = b"""<!doctype html><meta charset=utf-8><title>live page</title><body style="margin:0;background:#f4f4fa;font-family:sans-serif">
<h1 style="margin:0;padding:24px;background:#283c78;color:#fff">live page for the phone test</h1>
<input id=q style="position:absolute;left:440px;top:130px;width:400px;height:60px;font-size:32px" autocomplete=off
 oninput="fetch('/typed?v='+encodeURIComponent(this.value))" onkeydown="if(['Enter','Tab','Backspace','ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].includes(event.key))fetch('/key?k='+event.key)">
<button id=b style="position:absolute;left:440px;top:300px;width:400px;height:200px;font-size:40px" onclick="fetch('/clicked');this.textContent='clicked '+(window.n=(window.n||0)+1)">Click me</button>
<div style="height:4000px"></div>
<script>let st;addEventListener('scroll',()=>{clearTimeout(st);st=setTimeout(()=>fetch('/scrolled?y='+Math.round(scrollY)),150)})</script></body>"""
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        u = urllib.parse.urlparse(self.path); q = urllib.parse.parse_qs(u.query)
        line = {"/clicked": lambda: "click %f" % time.time(), "/typed": lambda: "typed " + q.get("v", [""])[0],
                "/key": lambda: "key " + q.get("k", [""])[0], "/scrolled": lambda: "scrolled " + q.get("y", ["0"])[0]}.get(u.path)
        if line:
            open(EVENTS, "a").write(line() + "\n"); self.send_response(204); self.end_headers(); return
        self.send_response(200); self.send_header("Content-Type", "text/html"); self.end_headers(); self.wfile.write(PAGE)
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", PORT), H).serve_forever()
