#!/usr/bin/env python3
"""One request to a running hub over its data/1 socket, with that hub's own token file: live.py BASE TOKEN_FILE METHOD [JSON]. Prints the reply. For checks on
a person's machines (with their go); never prints the token."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub
from link import rpc
hub = Hub(sys.argv[1], "")
hub.root = open(sys.argv[2]).read().strip()
print(json.dumps(rpc(hub, sys.argv[3], json.loads(sys.argv[4]) if len(sys.argv) > 4 else {})))
