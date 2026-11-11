#!/usr/bin/env python3
"""A tiny stdio MCP server for the MCP end-to-end (test/e2e/mcp/real.ts): one tool, `stamp {word}`, which writes what it was called with and the
PROBE_TOKEN it was started with (the vault's value, put in by Backplane's wrapper) to PROBE_OUT, one JSON line per call."""
import json, os, sys
def out(o): sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    try: m = json.loads(line)
    except Exception: continue
    mid, meth = m.get("id"), m.get("method", "")
    if meth == "initialize":
        out({"jsonrpc": "2.0", "id": mid, "result": {"protocolVersion": m.get("params", {}).get("protocolVersion", "2025-06-18"), "capabilities": {"tools": {}}, "serverInfo": {"name": "bp-probe", "version": "1.0"}}})
    elif meth == "tools/list":
        out({"jsonrpc": "2.0", "id": mid, "result": {"tools": [
            {"name": "stamp", "description": "Stamps a word (the e2e probe).", "inputSchema": {"type": "object", "properties": {"word": {"type": "string"}}, "required": ["word"]}},
            {"name": "forbidden", "description": "Turned off in Backplane: must never be called.", "inputSchema": {"type": "object", "properties": {}}}]}})
    elif meth == "tools/call":
        p = m.get("params", {}); name = p.get("name"); word = (p.get("arguments") or {}).get("word", "")
        with open(os.environ["PROBE_OUT"], "a") as f: f.write(json.dumps({"tool": name, "word": word, "token": os.environ.get("PROBE_TOKEN", ""), "pid": os.getpid()}) + "\n")
        out({"jsonrpc": "2.0", "id": mid, "result": {"content": [{"type": "text", "text": f"stamped {word}"}]}})
    elif mid is not None:
        out({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "not supported"}})
