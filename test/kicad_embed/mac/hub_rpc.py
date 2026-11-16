#!/usr/bin/python3
"""hub_rpc.mjs for a Mac without bun or node: hub_rpc.py PORT TOKEN_FILE '[{"method": .., "params": ..}, ...]' sends each step over the
hub's WebSocket (hub_ws.py) and prints {"sent": method} per step (the callers, libraries_row.py's turn.start, do not read replies)."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hub_ws
port, token = int(sys.argv[1]), open(sys.argv[2]).read().strip()
for st in json.loads(sys.argv[3]):
    hub_ws.send(port, token, st['method'], st.get('params', {}))
    print(json.dumps({'sent': st['method']}), flush=True)
