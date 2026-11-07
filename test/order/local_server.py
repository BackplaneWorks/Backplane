#!/usr/bin/env python3
"""The real order server (order/fabsrv) on a private port, for a test hub: the stand-in fab pack and a Stripe double in this process.

    python test/order/local_server.py --port 3871 --data DIR [--pilot] [--stripe-port N] [--export DIR]

--stripe-port fixes the Stripe double's port (a phone reaches it through a forwarded port); 0, the default, takes any free one.

--export writes an order's folder (a small board's gerbers and manifest.json) and exits. Point a test hub at the server with
BACKPLANE_ORDER_SERVER or the order.server setting. It never reaches a real service: it refuses to start unless Stripe's address is
loopback. The double is what order/fabsrv/pay.py calls, and opening a checkout's url pays it, as a test card would.
"""
import argparse, hashlib, io, json, os, pathlib, sys, threading, time, urllib.parse, zipfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUTLINE = "X0Y0D02*\nX20000000Y0D01*\nX20000000Y15000000D01*\nX0Y15000000D01*\nX0Y0D01*\n"
PADS = "X1000000Y1000000D03*\nX2000000Y1000000D03*\nX3000000Y1000000D03*\n"
DRILL = "M48\n;TYPE=PLATED\nMETRIC\nT1C0.300\n%\nT1\nX5.0Y5.0\nX6.0Y5.0\nM30\n"


def gerber(function, body):
    return f"%FSLAX46Y46*%\n%MOMM*%\n%TF.FileFunction,{function}*%\n%ADD10C,0.100000*%\nD10*\n{body}M02*\n"


def export(folder):
    b = io.BytesIO()
    with zipfile.ZipFile(b, "w") as z:
        z.writestr("b-Edge_Cuts.gm1", gerber("Profile,NP", OUTLINE))
        z.writestr("b-F_Cu.gtl", gerber("Copper,L1,Top", ""))
        z.writestr("b-B_Cu.gbl", gerber("Copper,L2,Bot", ""))
        z.writestr("b-F_Paste.gtp", gerber("Paste,Top", PADS))
        z.writestr("b-PTH.drl", DRILL)
    d = pathlib.Path(folder); (d / "main").mkdir(parents=True, exist_ok=True); (d / "main/gerbers.zip").write_bytes(b.getvalue())
    man = {"spec": 1, "title": "Test", "ship_to": "US", "parts": [{"name": "Main board", "kind": "pcb", "qty": 5, "options": {"layers": 2}, "files": {"gerbers": "main/gerbers.zip"}, "assemble": 0}],
           "files": {"main/gerbers.zip": hashlib.sha256(b.getvalue()).hexdigest()}}
    (d / "manifest.json").write_text(json.dumps(man))


class Stripe(BaseHTTPRequestHandler):
    sessions = {}

    def log_message(self, *a):
        pass

    def send(self, code, body, headers=()):
        t = json.dumps(body).encode()
        self.send_response(code)
        for k, v in headers:
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(t))); self.end_headers(); self.wfile.write(t)

    def do_POST(self):
        form = urllib.parse.parse_qs(self.rfile.read(int(self.headers.get("content-length", 0))).decode())
        sid = f"cs_test_{len(self.sessions) + 1}"
        amount = sum(int(v[0]) for k, v in form.items() if k.endswith("[unit_amount]"))
        s = {"id": sid, "url": f"http://127.0.0.1:{self.server.server_port}/pay/{sid}", "status": "open", "payment_status": "unpaid", "amount_total": amount, "success": form["success_url"][0],
             "customer_details": {"email": "test@example.invalid", "phone": ""}}
        self.sessions[sid] = s; self.send(200, s)

    def do_GET(self):
        p = self.path.strip("/").split("/")
        s = self.sessions.get(p[-1])
        if s is None:
            return self.send(404, {"error": "no such session"})
        if p[0] == "pay":  # the test card: paid, and back to the order server
            s.update(status="complete", payment_status="paid")
            return self.send(303, {}, [("Location", s["success"].replace("{CHECKOUT_SESSION_ID}", s["id"]))])
        self.send(200, s)


def main():
    a = argparse.ArgumentParser(); a.add_argument("--port", type=int, default=3871); a.add_argument("--data"); a.add_argument("--pilot", action="store_true"); a.add_argument("--export"); a.add_argument("--stripe-port", type=int, default=0)
    o = a.parse_args()
    if o.export:
        return export(o.export)
    stripe = ThreadingHTTPServer(("127.0.0.1", o.stripe_port), Stripe); threading.Thread(target=stripe.serve_forever, daemon=True).start()
    os.environ.update({"FAB_DATA": o.data, "FAB_VENDOR": str(ROOT / "order/tests/fakevendor"), "FAB_ENV_FILE": "/nonexistent", "FAB_ADMIN_TOKEN": "admin-token-for-local-tests-0123",
                       "FAB_PUBLIC_URL": f"http://127.0.0.1:{o.port}", "STRIPE_KEY": "sk_test_double_not_a_real_key", "FAB_PILOT": "1" if o.pilot else "0"})
    sys.path.insert(0, str(ROOT / "order"))
    from fabsrv import app as appmod, pay
    pay.API = f"http://127.0.0.1:{stripe.server_port}"
    assert pay.API.startswith("http://127.0.0.1:"), "the Stripe double must be local"
    import uvicorn
    uvicorn.run(appmod.app, host="127.0.0.1", port=o.port, log_level="warning")


if __name__ == "__main__":
    main()
