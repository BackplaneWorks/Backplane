#!/usr/bin/env python3
"""Repro (5): fp_value_partial.py MCP_URL|URLFILE PROJECT_DIR (a copy of the stickhub fixture). Prints R2's Value property after a partial
update (text only) and R1's after an update that sends the whole field."""
import json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
m = Mcp(sys.argv[1]); d = Doc(m, sys.argv[2] + "/StickHub.kicad_pcb")
fps = {ref(f): f for f in d.items("KOT_PCB_FOOTPRINT")}
whole = json.loads(json.dumps(fps["R1"]["value_field"])); whole["text"]["text"]["text"] = "10k0"
d.begin()
print("partial:", d.update([{"@type": BT + "FootprintInstance", "id": fps["R2"]["id"], "value_field": {"text": {"text": {"text": "10k1"}}}}])["updated_items"][0]["status"])
print("whole field:", d.update([{"@type": BT + "FootprintInstance", "id": fps["R1"]["id"], "value_field": whole}])["updated_items"][0]["status"])
d.end("repro 5"); d.save()
t = open(sys.argv[2] + "/StickHub.kicad_pcb").read()
for v in ("10k1", "10k0"):
    i = t.index('(property "Value" "%s"' % v); print(re.sub(r"\s+", " ", t[i:i + 260]))
