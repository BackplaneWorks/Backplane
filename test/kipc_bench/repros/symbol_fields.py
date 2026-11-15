#!/usr/bin/env python3
"""Repro (4): symbol_fields.py MCP_URL|URLFILE PROJECT_DIR (a copy of the pic_programmer fixture). Creates Device:R three ways in a commit
that is dropped, and prints where KiCad put each one's reference and value."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
m = Mcp(sys.argv[1]); d = Doc(m, sys.argv[2] + "/pic_programmer.kicad_sch"); L = {"library_nickname": "Device", "entry_name": "R"}
def sym(x, extra): return {"@type": ST + "SchematicSymbolInstance", "position": xy(x, 30.48), "definition": {"id": L}, "lib_id": L, "unit": {"unit": 1}, **extra}
items = [sym(265.43, {"reference_field": {"name": "Reference", "text": {"text": "R92"}}, "value_field": {"name": "Value", "text": {"text": "2k"}}}),
    sym(260.35, {"fields_autoplaced": True, "reference_field": {"name": "Reference", "visible": True, "allow_auto_place": True, "text": {"text": "R91", "attributes": {"visible": True}}},
        "value_field": {"name": "Value", "visible": True, "allow_auto_place": True, "text": {"text": "1k", "attributes": {"visible": True}}}}),
    sym(270.51, {})]
d.begin()
for n, c in enumerate(d.create(items)["created_items"], 1):
    it = c["item"]; print(n, c["status"], [(it[k]["text"]["text"], it[k]["text"].get("position"), "field visible:", it[k].get("visible")) for k in ("reference_field", "value_field")], "autoplaced:", it.get("fields_autoplaced"))
d.ipc("EndCommit", {"header": d.h, "id": d.cid, "action": "CMA_DROP"})
