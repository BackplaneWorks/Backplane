#!/usr/bin/env python3
"""A KiCad HTTP library server (REST API v1) with one category and two parts, for issue #1192: one whose symbol is Device:R and one
whose symbol does not exist (Device:NoSuchSymbol).

  httplib_stub.py PORTFILE    listens on 127.0.0.1 at a free port, writes the port to PORTFILE, serves until killed;
                              each request's path is appended to PORTFILE.log
KiCad asks: <root>/ (the endpoints), <root>/categories.json, <root>/parts/category/<id>.json, <root>/parts/<id>.json.
"""
import json, sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PART_NAME = "470R 1206 1% 250mW 200V"
CATEGORIES = [{"id": "16", "name": "Resistors", "description": "Resistors"}]
MISSING_NAME = "Missing symbol part"
PARTS = [{"id": "1", "name": PART_NAME, "description": "470R thick film resistor"},
         {"id": "2", "name": MISSING_NAME, "description": "a part whose symbol does not exist"}]
PART = {
    "id": "1", "name": PART_NAME, "symbolIdStr": "Device:R",
    "exclude_from_bom": "False", "exclude_from_board": "False", "exclude_from_sim": "True",
    "fields": {
        "footprint": {"value": "Resistor_SMD:R_1206_3216Metric", "visible": "False"},
        "datasheet": {"value": "https://example.com/r1206.pdf", "visible": "False"},
        "value": {"value": "470R"},
        "reference": {"value": "R"},
        "description": {"value": "470R 1% 250mW 200V 1206", "visible": "False"},
        "keywords": {"value": "resistor 470R", "visible": "False"},
        "MPN": {"value": "RC1206FR-07470RL", "visible": "False"},
        "Manufacturer": {"value": "Example Resistors", "visible": "False"},
    },
}
MISSING = {
    "id": "2", "name": MISSING_NAME, "symbolIdStr": "Device:NoSuchSymbol",
    "exclude_from_bom": "False", "exclude_from_board": "False", "exclude_from_sim": "True",
    "fields": {"value": {"value": "X"}, "reference": {"value": "U"}, "MPN": {"value": "NO-SUCH-1", "visible": "False"}},
}


class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_GET(self):
        with open(sys.argv[1] + ".log", "a") as f: f.write(self.path + "\n")
        p = self.path.split("?")[0].rstrip("/")
        body = None
        if p == "/v1": body = {"categories": "", "parts": ""}
        elif p == "/v1/categories.json": body = CATEGORIES
        elif p == "/v1/parts/category/16.json": body = PARTS
        elif p == "/v1/parts/1.json": body = PART
        elif p == "/v1/parts/2.json": body = MISSING
        if body is None:
            self.send_response(404); self.end_headers(); return
        data = json.dumps(body).encode()
        self.send_response(200); self.send_header("content-type", "application/json"); self.send_header("content-length", str(len(data)))
        self.end_headers(); self.wfile.write(data)


s = ThreadingHTTPServer(("127.0.0.1", 0), H)
open(sys.argv[1] + ".tmp", "w").write(str(s.server_address[1]))
import os; os.rename(sys.argv[1] + ".tmp", sys.argv[1])
s.serve_forever()
