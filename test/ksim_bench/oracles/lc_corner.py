import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from osim import *
def run(d):
    d.exclude(["J1", "J2", "TP1", "H1", "H2", "H3", "H4"]); d.source("VSIN", "V1", "IN", "GND", dc=0, ampl=1, f="1k", ac=1); d.load("RL1", "8", "OUT", "GND")
    r = d.run(".ac dec 400 100 1meg", ["V(/OUT)", "V(/IN)"]); fs = [x.real for x in d.vec(r, "frequency")]
    f3, pk = refs.corner(fs, [a / b for a, b in zip(d.vec(r, "V(/OUT)"), d.vec(r, "V(/IN)"))]); return {"f3db_hz": f3, "peak_db": pk}
main(run)
