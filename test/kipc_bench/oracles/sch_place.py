import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
def run(m, cwd):
    d = Doc(m, cwd + "/pic_programmer.kicad_sch")
    d.begin()
    r = d.create([{"@type": ST + "SchematicSymbolInstance", "position": xy(250.19, 60.96), "definition": {"id": {"library_nickname": "Connector", "entry_name": "TestPoint"}}, "lib_id": {"library_nickname": "Connector", "entry_name": "TestPoint"},
          "reference_field": {"name": "Reference", "text": {"text": "TP1"}}, "value_field": {"name": "Value", "text": {"text": "CLK"}}, "unit": {"unit": 1}},
        {"@type": ST + "SchematicLine", "type": "SLT_WIRE", "start": xy(250.19, 60.96), "end": xy(250.19, 66.04)},
        {"@type": ST + "LocalLabel", "position": xy(250.19, 66.04), "text": {"position": xy(250.19, 66.04), "text": "PC-CLOCK-OUT", "attributes": {"size": xy(1.27, 1.27)}}}])
    print(json.dumps([{"s": c["status"], "i": {k: v for k, v in c.get("item", {}).items() if k != "definition"}} for c in r["created_items"]])[:3000]); d.end("TP1 on PC-CLOCK-OUT"); print(d.save())
main(run)
