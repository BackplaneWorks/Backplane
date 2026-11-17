import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from osim import *
def run(d):
    d.exclude(["J1", "J2", "TP1", "H1", "H2", "H3", "H4"]); d.source("VSIN", "V1", "IN", "GND", dc=0, ampl=1, f="1k", ac=1); best = None
    for m in refs.E24:  # one decade is enough: 1 nF gives 6.3 kHz, 10 nF 2.2 kHz
        c = m * 1e-9; r = d.run(".ac dec 400 10 1meg", ["V(/OUT)", "V(/IN)"], overrides=[("C2", "", f"{c:.4g}")])
        f3 = refs.corner([x.real for x in d.vec(r, "frequency")], [a / b for a, b in zip(d.vec(r, "V(/OUT)"), d.vec(r, "V(/IN)"))])[0]
        if best is None or abs(f3 - 4000) < abs(best[1] - 4000): best = (m, f3)
    c2 = next(s for s in d.items("KOT_SCH_SYMBOL") if s["reference_field"]["text"]["text"] == "C2"); c2.pop("definition", None); c2["value_field"]["text"]["text"] = f"{best[0]:g}n"
    d.begin(); print(json.dumps(d.update([c2]))[:200]); d.end("C2 chosen"); return {"c2_nf": best[0], "f3db_hz": best[1]}
main(run)
