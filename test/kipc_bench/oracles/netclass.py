import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
def run(m, cwd):
    d = Doc(m, cwd + "/StickHub.kicad_pcb"); p = d.doc["project"]; mm = lambda v: {"value_nm": nm(v)}
    d.ipc("SetNetClasses", {"project": p, "merge_mode": "MMM_MERGE", "net_classes": [{"name": "USB", "type": "NCT_EXPLICIT", "board": {"clearance": mm(0.15), "track_width": mm(0.2), "diff_pair_track_width": mm(0.2), "diff_pair_gap": mm(0.18)}}]})
    d.ipc("SetNetClassAssignments", {"project": p, "merge_mode": "MMM_MERGE", "pattern_assignments": [{"pattern": "/U*D+", "netclass": "USB"}, {"pattern": "/U*D-", "netclass": "USB"}, {"pattern": "/D+", "netclass": "USB"}, {"pattern": "/D-", "netclass": "USB"}]})
    print(d.ipc("GetNetClassAssignments", {"project": p})); print(d.save())
main(run)
