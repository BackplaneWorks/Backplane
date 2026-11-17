import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from ksimo import *
def run(k):
    r = k.sim({"exclude": "no_model"}, {"source": {"type": "sin", "pos": "IN", "ac": 1}},
              {"tune": "C2", "measure": {"corner": "V(OUT)/V(IN)"}, "target": 4000, "from": "1n", "to": "10n", "series": "E24", "run": ".ac dec 400 10 1meg", "write": True})
    t = r[-1]["tune"]; import checks; return {"c2_nf": checks.si(t["value"]) * 1e9, "f3db_hz": t["measure"]}
main(run)
