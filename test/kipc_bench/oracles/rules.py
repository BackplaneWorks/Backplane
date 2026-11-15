import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
def run(m, cwd):
    d = Doc(m, cwd + "/StickHub.kicad_pcb")
    r = d.ipc("kiapi.backplane.coedit.EditCustomRules", {"board": d.doc, "upsert": [
        {"name": "usb_clearance", "condition": "A.NetClass == 'USB'", "constraints": [{"type": "CRCT_CLEARANCE", "numeric": {"min": int(nm(0.2))}}]},
        {"name": "hv_cap_courtyard", "condition": "A.Reference == 'C38'", "constraints": [{"type": "CRCT_COURTYARD_CLEARANCE", "numeric": {"min": int(nm(0.5))}}]}]})
    print(json.dumps(r)[:1500]); print(d.save())
main(run)
