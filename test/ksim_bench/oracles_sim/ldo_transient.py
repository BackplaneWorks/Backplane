import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from ksimo import *
def run(k):
    r = k.sim({"exclude": "no_model"}, {"model": "U1", "lib": "models/BP758.lib", "pins": {"1": "in", "2": "gnd", "3": "en", "4": "fb", "5": "out"}},
              {"source": {"type": "dc", "pos": "VIN", "value": 5}},
              {"source": {"type": "pulse", "current": True, "pos": "VOUT", "params": {"y1": "10m", "y2": "200m", "td": "2m", "tr": "1u", "tf": "1u", "tw": "2m", "per": "10m"}}},
              {"run": ".tran 50n 4m 1.9m 50n", "measures": {"pre": {"at": "V(VOUT)", "x": 1.999e-3}, "low": {"min": "V(VOUT)", "from": 2e-3, "to": 4e-3},
                                                          "rec": {"settling_time": "V(VOUT)", "t0": 2e-3, "tol": 5e-3, "from": 2e-3, "to": 3.99e-3}}},
              {"loopgain": "U1.4", "set": {"I1.y1": "200m"}})
    m = r[-2]["run"]["measures"]; return {"undershoot_mv": (m["pre"] - m["low"]) * 1e3, "recover_us": m["rec"] * 1e6, "pm_deg": r[-1]["loopgain"]["phase_margin"]}
main(run)
