import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from ksimo import *
def run(k):
    # the 8 ohm load is a netlist line of the run: kicad_sim has no op that puts a load on the schematic
    r = k.sim({"exclude": "no_model"}, {"source": {"type": "sin", "pos": "IN", "ac": 1}},
              {"run": ".ac dec 400 100 1meg", "directives": ["Rload /OUT 0 8"], "measures": {"f3": {"corner": "V(OUT)/V(IN)", "ref": "dc"}, "pk": "peak V(OUT)/V(IN)", "dc": "dc_gain V(OUT)/V(IN)"}})
    m = r[-1]["run"]["measures"]; return {"f3db_hz": m["f3"], "peak_db": m["pk"]["db"] - m["dc"]}
main(run)
