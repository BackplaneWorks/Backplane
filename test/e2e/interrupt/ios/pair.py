#!/usr/bin/env python3
"""pair.py: hub B (3982) pairs with hub A, reached through the relay (3991); A approves. Prints the link id."""
import sys, time, json
sys.path.insert(0, sys.argv[1])
from proto import Hub
from link import rpc, wait
R = sys.argv[2]
A = Hub("http://127.0.0.1:3981", open(f"{R}/homeA/token").read().strip())
B = Hub("http://127.0.0.1:3982", open(f"{R}/homeB/token").read().strip())
code = rpc(A, "machines.code"); print("code", json.dumps(code)[:120])
p = rpc(B, "machines.pair", {"address": "http://127.0.0.1:3991", "code": code["code"]}); print("pair", json.dumps(p)[:200])
req = wait(lambda: (rpc(A, "machines.requests") or {}).get("requests")); print("req", json.dumps(req)[:200])
print("approve", json.dumps(rpc(A, "machines.approve", {"request": req[0]["id"]}))[:200])
st = wait(lambda: (lambda r: r if isinstance(r, dict) and r.get("state") in ("linked", "refused") else None)(rpc(B, "machines.attempt", {"attempt": p["attempt"]})))
print("state", json.dumps(st)[:300])
