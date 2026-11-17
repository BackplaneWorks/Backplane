import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from osim import *
def run(d):
    d.exclude(["J1", "J2", "TP1", "H1", "H2", "H3", "H4"]); d.source("VDC", "V1", "VBUS", "GND", dc=24); d.source("VDC", "V2", "+3V3", "GND", dc=3.3)
    nom = {"R1": (180e3, 0.01), "R2": (22e3, 0.01), "R3": (220e3, 0.05)}; vals = []
    for k in range(8):
        ov = [(n, "", v * (1 + t * (1 if k >> i & 1 else -1))) for i, (n, (v, t)) in enumerate(nom.items())]; vals.append(d.value(d.run(".op", overrides=ov), "V(/ADC)"))
    return {"vmin_v": min(vals), "vmax_v": max(vals)}
main(run)
