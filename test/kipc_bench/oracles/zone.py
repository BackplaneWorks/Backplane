import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
def run(m, cwd):
    d = Doc(m, cwd + "/StickHub.kicad_pcb")
    z = [z for z in d.items("KOT_PCB_ZONE") if z.get("copper_settings", {}).get("net", {}).get("name") == "+1V8"][0]
    z.pop("filled_polygons", None); z["name"] = "PWR_1V8"; z["copper_settings"]["clearance"] = {"value_nm": nm(0.2)}; z["copper_settings"]["min_thickness"] = {"value_nm": nm(0.2)}
    d.begin(); r = d.update([z]); print(json.dumps(r)[:300]); d.end("1V8 zone"); print(d.ipc(B + "RefillZones", {"board": d.doc, "zones": [z["id"]]})); print(d.save())
main(run)
