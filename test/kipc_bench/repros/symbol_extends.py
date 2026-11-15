#!/usr/bin/env python3
"""Repros (6) and (7): symbol_extends.py MCP_URL|URLFILE PROJECT_DIR CLI (a copy of the pic_programmer fixture; CLI is the runtime's kicad-cli).
(6) CreateItems of a library symbol that `extends` another, by its library id, saves a sheet copy ERC calls lib_symbol_mismatch.
(7) A Footprint sent in user_fields is answered ISC_OK and not written. Both are saved, then read from the file and from ERC."""
import json, os, subprocess, sys, tempfile
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
m = Mcp(sys.argv[1]); sch = sys.argv[2] + "/pic_programmer.kicad_sch"; d = Doc(m, sch)
def sym(lib, entry, x, ref, extra=None):
    L = {"library_nickname": lib, "entry_name": entry}
    return {"@type": ST + "SchematicSymbolInstance", "position": xy(x, 33.02), "definition": {"id": L}, "lib_id": L, "unit": {"unit": 1}, "reference_field": {"name": "Reference", "text": {"text": ref}}, **(extra or {})}
fp = {"name": "Footprint", "visible": False, "text": {"text": "Capacitor_SMD:C_0805_2012Metric", "position": xy(226.06, 33.02), "attributes": {"visible": False}}}
d.begin()
r = d.create([sym("Regulator_Linear", "AMS1117-3.3", 241.3, "U91"), sym("Device", "C", 226.06, "C91", {"user_fields": [fp]})])
print("create:", [c["status"] for c in r["created_items"]])
d.end("repro"); d.save()
t = open(sch).read(); i = t.find('(property "Reference" "C91"'); k = t.rfind("\n\t(symbol", 0, i)
print("(7) C91's Footprint property in the saved file:", [l.strip() for l in t[k:i + 2500].splitlines() if '"Footprint"' in l][:1])
with tempfile.TemporaryDirectory() as tmp:
    subprocess.run([sys.argv[3], "sch", "erc", "--format", "json", "-o", tmp + "/e.json", sch], capture_output=True)
    v = [x for s in json.load(open(tmp + "/e.json"))["sheets"] for x in s["violations"] if x["type"] == "lib_symbol_mismatch"]
print("(6) lib_symbol_mismatch:", [[i["description"] for i in x["items"]] for x in v])
