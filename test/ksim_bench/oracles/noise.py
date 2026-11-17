import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from osim import *
def run(d):
    preamp_setup(d); r = d.run(".noise v(/AOUT) V1 dec 200 20 20k"); return {"noise_uvrms": d.value(r, "onoise_total") * 1e6}
main(run)
