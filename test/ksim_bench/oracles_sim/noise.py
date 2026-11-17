import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from ksimo import *
def run(k):
    r = k.sim({"exclude": "no_model"}, {"model": "U1", "lib": "models/BP321.lib", "pins": {"1": "inp", "3": "inn", "5": "vcc", "2": "vee", "4": "out"}}, {"source": {"type": "sin", "pos": "IN", "ac": 1}}, {"source": {"type": "dc", "pos": "+5V", "value": 5}}, {"run": ".noise v(/AOUT) V1 dec 200 20 20k", "measures": {"n": {"noise_rms": "onoise_spectrum", "from": 20, "to": 20000}}})
    return {"noise_uvrms": r[-1]["run"]["measures"]["n"] * 1e6}
main(run)
