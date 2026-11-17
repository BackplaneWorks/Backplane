import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from ksimo import *
def run(k):
    nom = {"R1": (180e3, 0.01), "R2": (22e3, 0.01), "R3": (220e3, 0.05)}  # read from the Tolerance fields by hand: kicad_sim has no worst case
    runs = [{"run": ".op", "set": {n: v * (1 + t * (1 if c >> i & 1 else -1)) for i, (n, (v, t)) in enumerate(nom.items())}, "signals": ["V(ADC)"]} for c in range(8)]
    r = k.sim({"exclude": "no_model"}, {"source": {"type": "dc", "pos": "VBUS", "value": 24}}, {"source": {"type": "dc", "pos": "+3V3", "value": 3.3}}, *runs)
    print(json.dumps(r[3])); vals = [next(v for kk, v in json.loads(json.dumps(x["run"])).get("op", x["run"].get("values", {})).items() if "adc" in kk.lower()) for x in r[3:]]
    return {"vmin_v": min(vals), "vmax_v": max(vals)}
main(run)
