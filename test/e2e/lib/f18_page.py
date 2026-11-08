"""A local test page for the Browser tab rows (F18): big coloured targets the driver finds by colour on a screenshot, and every reaction of the page is reported back to
this server as a request (/ev/<name>), so a check passes from what the page did, not from what the driver clicked.
  serve() -> (port, events)   events: a list of (time, name)"""
import http.server, threading, time, urllib.parse
PAGE = b"""<!doctype html><meta charset=utf-8><title>f18-start</title>
<style>body{margin:0;background:#fff;font:20px sans-serif;height:3600px}
#btn{position:absolute;left:80px;top:20px;width:300px;height:90px;background:#00cc66;border:0;font-size:24px}
#inp{position:absolute;left:80px;top:130px;width:500px;height:70px;background:#ffff66;border:2px solid #333;font-size:30px}
#pop{position:absolute;left:80px;top:230px;width:300px;height:90px;background:#ff8800;border:0;font-size:24px}
#dl{position:absolute;left:80px;top:340px;width:300px;height:90px;background:#cc00cc;color:#fff;font-size:24px;line-height:90px;text-align:center;text-decoration:none}
#out{position:absolute;left:80px;top:450px}
#end{position:absolute;left:0;top:3400px;width:100%;height:100px;background:#0033cc}</style>
<button id=btn>Click me</button><input id=inp placeholder=type><button id=pop>popup</button><a id=dl href=/file.txt>download</a><div id=out>idle</div><div id=end></div>
<script>
const ev=n=>fetch('/ev/'+encodeURIComponent(n),{keepalive:true}).catch(()=>{});
ev('load');
btn.onclick=()=>{document.title='clicked';out.textContent='clicked';ev('click')};
inp.oninput=()=>{document.title='typed:'+inp.value;ev('input:'+inp.value)};
pop.onclick=()=>{ev('popup-click');window.open('/popup.html','_blank')};
dl.addEventListener('click',()=>ev('download-click'));
let last=0;addEventListener('scroll',()=>{const y=Math.round(scrollY);if(Math.abs(y-last)>40){last=y;ev('scroll:'+y)}});
addEventListener('message',e=>ev('message:'+e.data));
</script>"""
POPUP = b"""<!doctype html><title>popup</title><body style="background:#ffcc00"><p style="font-size:40px">popup page</p><script>
fetch('/ev/popup-loaded',{keepalive:true});if(window.opener){window.opener.postMessage("hi-from-popup","*");fetch('/ev/popup-has-opener',{keepalive:true})}</script>"""
def serve():
    events = []
    class H(http.server.BaseHTTPRequestHandler):
        def log_message(self, *a): pass
        def _send(self, code, body, ctype='text/html', extra=()):
            self.send_response(code); self.send_header('content-type', ctype); self.send_header('content-length', str(len(body)))
            for k, v in extra: self.send_header(k, v)
            self.end_headers(); self.wfile.write(body)
        def do_GET(self):
            p = urllib.parse.urlparse(self.path).path
            if p.startswith('/ev/'): events.append((time.time(), urllib.parse.unquote(p[4:]))); return self._send(200, b'ok', 'text/plain')
            if p in ('/', '/index.html'): return self._send(200, PAGE)
            if p == '/popup.html': return self._send(200, POPUP)
            if p == '/file.txt': events.append((time.time(), 'download-get')); return self._send(200, b'f18 download body\n', 'application/octet-stream', [('content-disposition', 'attachment; filename="f18-file.txt"')])
            self._send(404, b'no', 'text/plain')
    srv = http.server.ThreadingHTTPServer(('127.0.0.1', 0), H); threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv.server_address[1], events
