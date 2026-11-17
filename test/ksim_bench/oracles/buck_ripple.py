import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from osim import *
import shutil, tempfile, mkfix
def run(d):
    os.makedirs(d.cwd + "/sim/models", exist_ok=True); t = tempfile.mkdtemp(); lib = mkfix.vendor(t); assert lib, "the vendor's model did not download"
    shutil.copy(lib, d.cwd + "/sim/models/"); open(d.cwd + "/sim/models/parts.lib", "w").write(mkfix.PARTS_LIB); shutil.rmtree(t)
    d.exclude(["J1", "J2", "TP1", "H1", "H2", "H3", "H4"])
    d.library("U1", "sim/models/" + mkfix.VENDOR["member"], mkfix.VENDOR["subckt"], {"1": "BOOT", "6": "COMP", "3": "EN", "7": "GND", "8": "PH", "4": "SS", "2": "VIN", "5": "VSENSE"})
    d.library("D1", "sim/models/parts.lib", "MBR340", {"1": "2", "2": "1"}); d.library("C2", "sim/models/parts.lib", "C220U_ESR60M", {"1": "p", "2": "n"})
    d.source("VDC", "V1", "VIN", "GND", dc=12); d.load("RL1", "1.65", "VOUT", "GND")
    # with KiCad's current probes in the netlist (.probe alli) this model stops at 56 us with "time step too small": they are left out
    r = d.run(".tran 100n 8m", measures=["tran vpp PP v(/vout) from=7m to=8m"], directives=[refs.BUCK_OPTS], options={"save_all_currents": False})
    return {"ripple_mv": d.measure(r, "vpp") * 1e3}
main(run)
