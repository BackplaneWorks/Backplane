#!/usr/bin/env python3
"""The hub's MCP tools by hand or from an oracle: mcp.py URL|URLFILE TOOL 'ARGS_JSON' -> the tool's text; module: Mcp(url).call(tool, args)."""
import json, os, sys, urllib.request
class Mcp:
    def __init__(self, url): self.url = open(url).read().strip() if os.path.exists(url) else url; self.n = 0; self.calls = 0; self.bytes = 0
    def raw(self, method, params):
        self.n += 1
        r = urllib.request.Request(self.url, json.dumps({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params}).encode(), {"content-type": "application/json", "accept": "application/json"})
        return json.loads(urllib.request.urlopen(r, timeout=300).read())
    def call(self, tool, args):
        x = self.raw("tools/call", {"name": tool, "arguments": args}).get("result", {})
        t = "".join(c.get("text", "") for c in x.get("content") or [])
        self.calls += 1; self.bytes += len(t)
        try: b = json.loads(t)
        except Exception: b = t
        if x.get("isError"): raise RuntimeError(str(b)[:2000])
        return b
    def ipc(self, path, ty, msg=None):
        if "." not in ty: ty = "kiapi.common.commands." + ty
        return self.call("kicad_ipc", {"path": path, "type": ty, "message": msg or {}}).get("message", {})
if __name__ == "__main__":
    m = Mcp(sys.argv[1])
    if sys.argv[2] == "list": print(json.dumps(m.raw("tools/list", {}))); sys.exit()
    try: r = m.call(sys.argv[2], json.loads(sys.argv[3]) if len(sys.argv) > 3 else {})
    except RuntimeError as e: print("ERROR", e); sys.exit(1)
    s = json.dumps(r); print(s if len(s) < int(os.environ.get("MAX", "3000")) else s[:int(os.environ.get("MAX", "3000"))] + f"... [{len(s)} chars]")
