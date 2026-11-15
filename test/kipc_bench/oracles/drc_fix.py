import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
def run(m, cwd):
    d = Doc(m, cwd + "/StickHub.kicad_pcb")
    t = [t for t in d.items("KOT_PCB_TRACE") if t["start"] == xy(146.2, 112.4) and t["end"] == xy(153.8, 112.4)]
    d.begin(); print(d.delete([t[0]["id"]["value"]])); d.end("stray track"); print(d.save())
main(run)
