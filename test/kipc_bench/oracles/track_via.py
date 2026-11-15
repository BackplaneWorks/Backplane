import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
def run(m, cwd):
    d = Doc(m, cwd + "/StickHub.kicad_pcb")
    d.begin()
    r = d.create([{"@type": BT + "Track", "start": xy(157.25, 100.25), "end": xy(157.25, 98.5), "width": {"value_nm": nm(0.25)}, "layer": "BL_B_Cu", "net": {"name": "GND"}},
        {"@type": BT + "Via", "position": xy(157.25, 98.5), "type": "VT_THROUGH", "net": {"name": "GND"}, "pad_stack": {"type": "PST_NORMAL", "layers": ["BL_F_Cu", "BL_B_Cu"], "drill": {"start_layer": "BL_F_Cu", "end_layer": "BL_B_Cu", "diameter": xy(0.3, 0.3), "shape": "DS_CIRCLE"}, "copper_layers": [{"layer": "BL_F_Cu", "shape": "PSS_CIRCLE", "size": xy(0.6, 0.6)}]}}])
    print(json.dumps(r)[:1500]); d.end("GND stub and via"); print(d.save())
main(run)
