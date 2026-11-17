import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from ksimo import *
def run(k):
    r = k.sim({"exclude": "no_model"}, {"model": "U1", "lib": "models/BP321.lib", "pins": {"1": "inp", "3": "inn", "5": "vcc", "2": "vee", "4": "out"}}, {"source": {"type": "sin", "pos": "IN", "ac": 1}}, {"source": {"type": "dc", "pos": "+5V", "value": 5}}, {"run": ".ac dec 100 100 10k", "measures": {"g": {"gain_at": "V(AOUT)/V(IN)", "x": 1000}}}, {"loopgain": "U1.3"})
    return {"gain_vv": 10 ** (r[-2]["run"]["measures"]["g"] / 20), "pm_deg": r[-1]["loopgain"]["phase_margin"]}
main(run)
