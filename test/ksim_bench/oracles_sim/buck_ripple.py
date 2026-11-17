import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from ksimo import *
import shutil, tempfile, mkfix
def run(k):
    t = tempfile.mkdtemp(); lib = mkfix.vendor(t); assert lib, "the vendor's model did not download"   # the fetch is the agent's; import keeps the file
    # the diode model the vendor's library names without defining, and the capacitor with its series resistance, are text the agent writes
    r = k.sim({"exclude": "no_model"},
              {"import": {"file": lib, "url": mkfix.VENDOR["url"], "licence": "(C) Texas Instruments, provided as is"}, "to": "U1", "name": mkfix.VENDOR["subckt"],
               "pins": {"1": "BOOT", "6": "COMP", "3": "EN", "7": "GND", "8": "PH", "4": "SS", "2": "VIN", "5": "VSENSE"}},
              {"import": {"text": mkfix.PARTS_LIB, "url": "written for this project", "as": "parts.lib"}, "to": "C2", "name": "C220U_ESR60M", "pins": {"1": "p", "2": "n"}},
              {"model": "D1", "lib": "sim/models/parts.lib", "name": "MBR340", "pins": {"1": "2", "2": "1"}},
              {"source": {"type": "dc", "pos": "VIN", "value": 12}},
              {"run": ".tran 100n 8m", "directives": ["Rload /VOUT 0 1.65"], "options": {"save_all_currents": False}, "timeout": 600,
               "measures": {"vpp": {"ripple": "V(VOUT)", "from": 7e-3, "to": 8e-3}}})
    shutil.rmtree(t); print(json.dumps(r[-1])); return {"ripple_mv": r[-1]["run"]["measures"]["vpp"] * 1e3}
main(run)
