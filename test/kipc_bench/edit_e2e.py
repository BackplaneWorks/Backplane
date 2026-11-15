#!/usr/bin/env python3
"""Every benchmark task as ONE kicad_edit call, against a real KiCad (bench.py e2e): the tool's end-to-end test. Run as an oracle: edit_e2e.py MCP_URL CWD,
the task being the folder's name after "e2e-"."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp import Mcp
OPS = {
 "move_rotate": ("StickHub.kicad_pcb", [{"move": ["LOGO1", "LOGO2", "LOGO3", "LOGO4"], "by": [0, 1]}, {"set": "J9", "rot": 90}]),
 "sch_fields": ("pic_programmer.kicad_sch", [{"set": ["R%d" % i for i in range(1, 8)], "value": "10k", "fields": {"MPN": "RC0603FR-0710KL"}}, {"set": ["C6", "C7"], "value": "100n", "fields": {"MPN": "CC0603KRX7R9BB104"}}]),
 "netclass": ("StickHub.kicad_pcb", [{"netclass": {"name": "USB", "clearance": 0.15, "track_width": 0.2, "diff_pair_width": 0.2, "diff_pair_gap": 0.18, "patterns": ["/U*D+", "/U*D-", "/D+", "/D-"]}}]),
 "sch_place": ("pic_programmer.kicad_sch", [{"add": [{"symbol": {"lib": "Connector:TestPoint", "ref": "TP1", "value": "CLK", "at": [250.19, 60.96]}}, {"wire": [[250.19, 60.96], [250.19, 66.04]]}, {"label": {"text": "PC-CLOCK-OUT", "at": [250.19, 66.04]}}]}]),
 "track_via": ("StickHub.kicad_pcb", [{"add": [{"track": {"from": [157.25, 100.25], "to": [157.25, 98.5], "width": 0.25, "layer": "B.Cu", "net": "GND"}}, {"via": {"at": [157.25, 98.5], "size": 0.6, "drill": 0.3, "net": "GND"}}]}]),
 "zone": ("StickHub.kicad_pcb", [{"set": {"kind": "zone", "net": "+1V8"}, "clearance": 0.2, "min_width": 0.2, "name": "PWR_1V8"}]),
 "rules": ("StickHub.kicad_pcb", [{"rule": [{"name": "usb_clearance", "condition": "A.NetClass == 'USB'", "constraints": [{"type": "clearance", "min": 0.2}]}, {"name": "hv_cap_courtyard", "condition": "A.Reference == 'C38'", "constraints": [{"type": "courtyard_clearance", "min": 0.5}]}]}]),
 "drc_fix": ("StickHub.kicad_pcb", [{"find": {"kind": "track", "near": [150, 112.4, 0.1]}}, {"delete": {"kind": "track", "from": [146.2, 112.4], "to": [153.8, 112.4]}}]),
}
m = Mcp(sys.argv[1]); cwd = sys.argv[2]; f, ops = OPS[os.path.basename(cwd).split("-", 1)[1]]
# kicad_ipc's short command names complete the name only: a misspelt field is still KiCad's error, never passed over
try: m.call("kicad_ipc", {"path": cwd + "/" + f, "type": "GetItems", "message": {"header": {"document": "$doc"}, "typos": ["KOT_PCB_VIA"]}}); print("FAIL: a misspelt field passed")
except RuntimeError as e: print("misspelt field refused:", str(e)[:90])
r = m.call("kicad_edit", {"path": cwd + "/" + f, "ops": ops, "message": "bench"}); print(json.dumps(r)[:600]); print("calls", m.calls, "bytes", m.bytes)
