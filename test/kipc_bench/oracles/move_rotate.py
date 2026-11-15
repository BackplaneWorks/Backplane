import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
def run(m, cwd):
    d = Doc(m, cwd + "/StickHub.kicad_pcb")
    fps = {ref(f): f for f in d.items("KOT_PCB_FOOTPRINT")}
    ups = []
    for r in ["LOGO1", "LOGO2", "LOGO3", "LOGO4"]:
        f = fps[r]; ups.append({"@type": BT + "FootprintInstance", "id": f["id"], "position": {"x_nm": f["position"]["x_nm"], "y_nm": str(int(f["position"]["y_nm"]) + 1000000)}, "orientation": f.get("orientation", {}), "layer": f["layer"]})
    f = fps["J9"]; ups.append({"@type": BT + "FootprintInstance", "id": f["id"], "position": f["position"], "orientation": {"value_degrees": 90}, "layer": f["layer"]})
    d.begin(); r = d.update(ups); print(json.dumps(r)[:400]); d.end("move logos, rotate J9"); print(d.save())
main(run)
