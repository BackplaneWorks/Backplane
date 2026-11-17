import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from osim import *
def run(d):
    d.exclude(["J1", "J2", "TP1", "H1", "H2", "H3", "H4"]); d.library("U1", "models/BP758.lib", "BP758", {"1": "in", "2": "gnd", "3": "en", "4": "fb", "5": "out"})
    d.source("VDC", "V1", "VIN", "GND", dc=5); d.source("IPULSE", "I1", "VOUT", "GND", y1="10m", y2="200m", td="2m", tr="1u", tf="1u", tw="2m", per="10m")
    r = d.run(".tran 50n 4m 1.9m 50n", ["V(/VOUT)"]); out = {k: v for k, v in refs.ldo_step(d.vec(r, "time"), d.vec(r, "V(/VOUT)")).items() if not k.startswith("_")}
    rt = os.path.dirname(os.path.dirname(os.environ["BACKPLANE_KICAD_CLI"])); out["pm_deg"] = refs.ldo((rt, d.netlist(), d.cwd))["pm_deg"]; return out
main(run)
