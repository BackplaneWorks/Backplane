#!/usr/bin/env python3
# A page for the Browser tab's live check: one big button; a click on it asks the server for /clicked, which writes a line to the file named by argv[2].
# usage: live_page.py PORT CLICKS_FILE
import http.server, sys, time
PORT, CLICKS = int(sys.argv[1]), sys.argv[2]
PAGE = b"""<!doctype html><meta charset=utf-8><title>live page</title><body style="margin:0;background:#f4f4fa;font-family:sans-serif">
<h1 style="margin:0;padding:24px;background:#283c78;color:#fff">live page for the phone test</h1>
<button id=b style="position:absolute;left:440px;top:300px;width:400px;height:200px;font-size:40px" onclick="fetch('/clicked');this.textContent='clicked '+(window.n=(window.n||0)+1)">Click me</button></body>"""
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith("/clicked"):
            open(CLICKS, "a").write("click %f\n" % time.time()); self.send_response(204); self.end_headers(); return
        self.send_response(200); self.send_header("Content-Type", "text/html"); self.end_headers(); self.wfile.write(PAGE)
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", PORT), H).serve_forever()
