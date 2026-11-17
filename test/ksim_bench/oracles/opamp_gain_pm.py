import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from osim import *
def run(d):
    preamp_setup(d); r = d.run(".ac lin 2 1k 1.0001k", ["V(/AOUT)", "V(/IN)"]); gain = abs(d.vec(r, "V(/AOUT)")[0] / d.vec(r, "V(/IN)")[0])
    # the margin: RunSimulation cannot open the loop for one run, so the netlist is taken, the loop opened at the inverting input, and ngspice run on it
    rt = os.path.dirname(os.path.dirname(os.environ["BACKPLANE_KICAD_CLI"])); pm = refs.opamp_gain_pm((rt, d.netlist(), d.cwd))["pm_deg"]
    return {"gain_vv": gain, "pm_deg": pm}
main(run)
