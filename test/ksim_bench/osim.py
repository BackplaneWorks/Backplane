"""What the simulation oracles (oracles/*.py) share: each task done by hand over raw `kicad_ipc` on a runtime with the simulation commands
(kiapi.backplane.sim), no model and no kicad_sim: parts left out of simulation, models assigned, sources and loads placed out of the BOM and off the
board, the schematic saved, the analyses run, and the answer printed as the task's RESULT line. bench.py e2e runs them and holds the result to the
same check as an agent's run."""
import json, os, sys, time
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, os.path.join(HERE, "..", "kipc_bench"))
from olib import Doc, Mcp, xy, ST
sys.path.insert(0, HERE); sys.path.insert(0, HERE + "/tools")
import mna, ngs, refs
SIM = "kiapi.backplane.sim."; G = 2.54
POWER = ("GND", "+5V", "+3V3", "+12V")
def at(i, j): return 30 * G + i * 12 * G, 20 * G + j * 12 * G   # mkfix.py's grid; row 3 is free on every fixture

class Sim(Doc):
    def __init__(self, m, cwd):
        self.cwd = cwd; super().__init__(m, os.path.join(cwd, next(f for f in sorted(os.listdir(cwd)) if f.endswith(".kicad_sch")))); self.col = 0; self.pwr = 90
    def sim(self, name, msg=None): return self.ipc(SIM + name, {"document": self.doc, **(msg or {})})
    def exclude(self, refs_):
        self.bad(self.sim("AssignSimulationModel", {"references": refs_, "exclude_from_sim": True}))  # a mounting hole too (a symbol without pins)
    def bad(self, r):
        e = [(x["reference"], x.get("error")) for x in r.get("results", []) if x.get("error")]
        if e: raise RuntimeError(f"AssignSimulationModel: {e}")
    def library(self, ref, lib, name, pins):
        self.bad(self.sim("AssignSimulationModel", {"references": [ref], "library": {"library": lib, "name": name}, "pins": [{"pin_number": p, "model_node": n} for p, n in pins.items()]}))
    def builtin(self, ref, device, ty, params):
        self.bad(self.sim("AssignSimulationModel", {"references": [ref], "builtin": {"device": device, "type": ty, "params": [{"name": k, "value": str(v)} for k, v in params.items()]}}))
    def _sym(self, lib, ref, value, x, y, sim_only):
        l, n = lib.split(":"); lid = {"library_nickname": l, "entry_name": n}
        return {"@type": ST + "SchematicSymbolInstance", "position": xy(x, y), "definition": {"id": lid}, "lib_id": lid, "reference_field": {"name": "Reference", "text": {"text": ref}},
                "value_field": {"name": "Value", "text": {"text": value}}, "unit": {"unit": 1},
                **({"attributes": {"exclude_from_bill_of_materials": True, "exclude_from_board": True, "exclude_from_position_files": True}} if sim_only else {})}
    def _net(self, items, net, x, y):
        if net in POWER: self.pwr += 1; items.append(self._sym("power:" + net, f"#PWR{self.pwr}", net, x, y, False))
        else: items.append({"@type": ST + "LocalLabel", "position": xy(x, y), "text": {"position": xy(x, y), "text": net, "attributes": {"size": xy(1.27, 1.27)}}})
    def two_pin(self, lib, ref, value, p, n, half):
        """a two-pin part for simulation only in the next free place of row 3 (pin 1 up, pin 2 down, `half` mm from the middle), each pin on its net"""
        x, y = at(self.col, 3); self.col += 1; items = [self._sym(lib, ref, value, x, y, True)]
        for net, s in ((p, -1), (n, 1)):
            a, b = y + s * half, y + s * (half + 2.54); items.append({"@type": ST + "SchematicLine", "type": "SLT_WIRE", "start": xy(x, a), "end": xy(x, b)}); self._net(items, net, x, b)
        self.begin(); r = self.create(items); self.end("for simulation: " + ref)
        st = [c.get("status", {}).get("code") for c in r.get("created_items", [])]
        if any(s not in (None, "ISC_OK") for s in st): raise RuntimeError(f"{ref}: {st} {json.dumps(r)[:300]}")
    def source(self, kind, ref, p, n, **params):
        """Simulation_SPICE:VDC/VSIN/IPULSE/...: placed, then given its parameters as KiCad's model dialog writes them"""
        self.two_pin("Simulation_SPICE:" + kind, ref, kind, p, n, 5.08); self.builtin(ref, kind[0], kind[1:], params)
    def load(self, ref, value, p, n): self.two_pin("Device:R", ref, value, p, n, 3.81)
    def run(self, command, signals=(), measures=(), directives=(), overrides=(), timeout_ms=600000, options=None):
        r = self.sim("RunSimulation", {"command": command, "signals": list(signals), "measures": list(measures), "directives": list(directives),
                                       "overrides": [{"reference": a, "field": b, "value": str(c)} for a, b, c in overrides], "timeout_ms": timeout_ms, "wait_ms": 1000, **({"options": options} if options else {})})
        while r.get("status") == "SIM_RUNNING": r = self.sim("GetSimulationResult", {"run_id": r["run_id"], "wait_ms": 1000})
        if r.get("status") != "SIM_OK": raise RuntimeError(f"{command}: {r.get('status')} {json.dumps(r.get('problems'))[:400]} {r.get('log', '')[-300:]}")
        print("ran", command.splitlines()[0], "points", r.get("points"), "ngspice ms", r.get("timings", {}).get("run_ms"), flush=True); return r
    def vec(self, r, name):
        v = next(v for v in r["vectors"] if name.lower() in (v.get("name", "").lower(), v.get("signal", "").lower()))
        return [complex(a, b) for a, b in zip(v["real"], v["imag"])] if v.get("complex") else list(v["real"])
    def value(self, r, name): return next(v["value"] for v in r["values"] if name.lower() in (v.get("name", "").lower(), v.get("signal", "").lower()))
    def measure(self, r, name): m = next(m for m in r["measures"] if m["name"].lower() == name.lower()); assert not m.get("failed"), m; return m["value"]
    def netlist(self): return self.sim("GetSimulationNetlist")["netlist"]
    def ngspice(self, deck, cmds, vecs):
        """a netlist changed by hand (a loop opened), run through the runtime's libngspice: RunSimulation cannot rewire a net for one run"""
        rt = os.path.dirname(os.path.dirname(os.environ["BACKPLANE_KICAD_CLI"])); return refs.run(rt, deck, cmds, vecs, self.cwd, "ltpsa")
def preamp_setup(d):
    d.exclude(["J1", "J2", "J3", "TP1", "H1", "H2", "H3", "H4"]); d.library("U1", "models/BP321.lib", "BP321", {"1": "inp", "3": "inn", "5": "vcc", "2": "vee", "4": "out"})
    d.source("VSIN", "V1", "IN", "GND", dc=0, ampl="1m", f="1k", ac=1); d.source("VDC", "V2", "+5V", "GND", dc=5)
def main(fn):
    m = Mcp(sys.argv[1]); d = Sim(m, sys.argv[2]); out = fn(d); print(d.save())
    print("calls", m.calls, "bytes", m.bytes); print("RESULT " + " ".join(f"{k}={v:.6g}" for k, v in out.items()))
