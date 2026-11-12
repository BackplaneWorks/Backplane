#!/usr/bin/env python3
"""A stand-in for the pcbparts MCP server (streamable HTTP, JSON answers) for the BOM panel's tests: no request leaves the machine.

Usage: pcbparts_stub.py PORT LOG
Parts it knows (by MPN or by fab number):
  E2E-FILE / C1001  5000 in stock, basic, 0.01 -> 0.008 at 100
  E2E-LIVE / C1002  0 in stock (only jlc_stock_check finds it), extended, 1.20
  E2E-LOW  / C1003  3 in stock (low for one board), extended, 0.50
mouser_get_part: E2E-LIVE is NRND, E2E-LOW obsolete, E2E-FILE states none.
jlc_stock_history: C1003 had 400 then 90 in October; the others no recorded change.
Every call is a line in LOG ({"tool", "args"}), so a test can count them.
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PARTS = {
    "E2E-FILE": {"lcsc": "C1001", "model": "E2E-FILE", "manufacturer": "Stub", "stock": 5000, "price": 0.01, "library_type": "basic", "preferred": False,
                 "prices": [{"qty": "1+", "price": 0.01}, {"qty": "100+", "price": 0.008}], "package": "0603"},
    "E2E-LIVE": {"lcsc": "C1002", "model": "E2E-LIVE", "manufacturer": "Stub", "stock": 0, "price": 1.2, "library_type": "extended", "preferred": False, "package": "0603"},
    "E2E-LOW": {"lcsc": "C1003", "model": "E2E-LOW", "manufacturer": "Stub", "stock": 3, "price": 0.5, "library_type": "extended", "preferred": False, "package": "0603"},
}
BY_FAB = {p["lcsc"]: p for p in PARTS.values()}
PORT, LOG = int(sys.argv[1]), sys.argv[2]


def call(tool, args):
    with open(LOG, "a") as f:
        f.write(json.dumps({"tool": tool, "args": args}) + "\n")
    if tool == "jlc_get_part":
        if args.get("lcsc"):
            p = BY_FAB.get(args["lcsc"].upper())
            return p if p else {"error": "not found", "results": []}
        p = PARTS.get(args.get("mpn", ""))
        # the in-stock catalogue only, as the real one
        return {"results": [p] if p and p["stock"] >= 10 else [], "total": 1 if p and p["stock"] >= 10 else 0}
    if tool == "jlc_stock_check":
        p = PARTS.get(args.get("query", ""))
        return {"results": [p] if p else [], "total": 1 if p else 0}
    if tool == "mouser_get_part":
        life = {"E2E-LIVE": "Not Recommended for New Designs", "E2E-LOW": "Obsolete"}
        return {"results": [{"mfr_part_number": m, "manufacturer": "Stub", "lifecycle": life.get(m)} for m in args.get("part_number", "").split("|") if m in PARTS]}
    if tool == "jlc_stock_history":
        if args.get("lcsc") == "C1003":
            return {"lcsc": "C1003", "events": [{"date": "2026-10-01", "type": "change", "stock_after": 400}, {"date": "2026-10-15", "type": "change", "stock_after": 90}, {"date": "2026-10-20", "type": "gone", "stock_after": None}]}
        return {"lcsc": args.get("lcsc"), "events": []}
    if tool == "jlc_find_alternatives":
        return {"alternatives": [PARTS["E2E-FILE"], PARTS["E2E-LOW"]]}
    return {"error": "unknown tool"}


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))) or b"{}")
        m, rid = body.get("method"), body.get("id")
        if rid is None:
            self.send_response(202); self.end_headers(); return
        if m == "initialize":
            res = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "pcbparts-stub", "version": "1"}}
        elif m == "tools/call":
            p = body.get("params", {})
            res = {"content": [{"type": "text", "text": json.dumps(call(p.get("name"), p.get("arguments", {})))}]}
        elif m == "tools/list":
            res = {"tools": []}
        else:
            res = {}
        out = json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Mcp-Session-Id", "stub")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)


ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
