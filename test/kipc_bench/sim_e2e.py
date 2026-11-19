#!/usr/bin/env python3
"""kicad_sim against a real headless KiCad with `sim-run` (docs/kicad-sim.md): every op on small generated schematics, the numbers
checked against closed form. No model: each task is a turn of the stand-in agent that runs this file with the hub's MCP url.
  sim_e2e.py run WORK [task ...]   on a hub started by hub.mjs with WORK: make the fixtures, run each task, read the log; exit 1 on a failure
  sim_e2e.py MCP_URL CWD           (the turn's side) the task named by the folder after "e2e-"
A circuit is parts with the net of every pin: each part a generic symbol with its pins in a row and a label on each pin, so nothing
depends on a symbol library or on wires. A net called GND is ngspice's node 0."""
import json, math, os, shutil, subprocess, sys, time, uuid
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
ROOT = "5b0d3a1e-0000-4000-8000-0000000000e2"
def uid(seed): return str(uuid.uuid5(uuid.UUID(ROOT), seed))

def write_project(folder, name, parts):
    """parts: (ref, value, nets, pin names or None, fields or None) -> folder/name.kicad_sch and .kicad_pro"""
    os.makedirs(folder, exist_ok=True); libs, body = {}, []
    for row, (ref, value, nets, pins, fields) in enumerate(parts):
        pins = pins or [str(i + 1) for i in range(len(nets))]; prefix = "".join(c for c in ref if not c.isdigit()); lib = f"simfix:{prefix}{len(pins)}{'_' + '_'.join(pins) if any(not p.isdigit() for p in pins) else ''}"
        ptext = "".join(f'\n        (pin passive line (at {2.54 * i:.2f} 0 90) (length 1.27) (name "{p}" (effects (font (size 1.27 1.27)))) (number "{i + 1}" (effects (font (size 1.27 1.27)))))' for i, p in enumerate(pins))
        libs[lib] = f'''
    (symbol "{lib}" (in_bom yes) (on_board yes)
      (property "Reference" "{prefix}" (at 0 2.54 0) (effects (font (size 1.27 1.27))))
      (property "Value" "{prefix}" (at 0 5.08 0) (effects (font (size 1.27 1.27))))
      (property "Footprint" "" (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
      (property "Datasheet" "" (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
      (symbol "{lib.split(':')[1]}_1_1"{ptext}
      )
    )'''
        x, y = 50.8, 25.4 + 12.7 * row
        props = "".join(f'\n    (property "{k}" "{v}" (at {x:.2f} {y - 2.54:.2f} 0) (effects (font (size 1.27 1.27)){"" if k in ("Reference", "Value") else " hide"}))' for k, v in [("Reference", ref), ("Value", value), ("Footprint", ""), ("Datasheet", ""), *(fields or {}).items()])
        pin_ids = "".join(f'\n    (pin "{i + 1}" (uuid {uid(f"{name}/{ref}/pin{i}")}))' for i in range(len(nets)))
        body.append(f'''
  (symbol (lib_id "{lib}") (at {x:.2f} {y:.2f} 0) (unit 1)
    (exclude_from_sim no) (in_bom yes) (on_board yes) (dnp no)
    (uuid {uid(f"{name}/{ref}")}){props}{pin_ids}
    (instances (project "{name}" (path "/{ROOT}" (reference "{ref}") (unit 1))))
  )''')
        for i, net in enumerate(nets):
            if net: body.append(f'\n  (label "{net}" (at {x + 2.54 * i:.2f} {y:.2f} 90) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid {uid(f"{name}/{ref}/label{i}")}))')
    open(f"{folder}/{name}.kicad_sch", "w").write(f'(kicad_sch (version 20231120) (generator "backplane_sim_e2e")\n  (uuid {ROOT})\n  (paper "A4")\n  (lib_symbols{"".join(libs.values())}\n  )\n{"".join(body)}\n  (sheet_instances (path "/" (page "1")))\n)\n')
    open(f"{folder}/{name}.kicad_pro", "w").write(json.dumps({"meta": {"filename": f"{name}.kicad_pro", "version": 1}}, indent=2) + "\n")

# R1 1k into C1 100n; a connector, a test point and an op-amp with no model hang on it; no source yet
RC = [("R1", "1k", ["in", "out"], None, None), ("C1", "100n", ["out", "GND"], None, None), ("J1", "Conn_01x02", ["in", "GND"], None, None), ("TP1", "TestPoint", ["out"], None, None),
      ("U1", "LM358", ["out", "buf", "GND"], ["INP", "OUT", "GND"], {"MPN": "LM358DR", "Manufacturer": "Texas Instruments"})]
DIV = [("V1", "dc 5 ac 1", ["in", "GND"], None, None), ("R1", "10k", ["in", "out"], None, None), ("R2", "10k", ["out", "GND"], None, None)]
# series L1 1m, R1 50, then C1 100n to ground: V(c) is a second-order low-pass (z = 0.25), V(a)-V(c) across R1 a band-pass (Q = 2)
RLC = [("L1", "1m", ["in", "a"], None, None), ("R1", "50", ["a", "c"], None, None), ("C1", "100n", ["c", "GND"], None, None)]
FAIL = [("V1", "dc 1", ["in", "GND"], None, None), ("C1", "1u", ["in", "mid"], None, None), ("C2", "1u", ["mid", "GND"], None, None)]
# a loop: E1 = 100 (V(ref) - V(fb)) -> R1 1k, C1 100n -> a unity buffer E2 -> R2 1k, C2 10n -> fb; V1 on ref has an AC value of its own
E = lambda gain: {"Sim.Device": "SPICE", "Sim.Params": 'type=\\"E\\" model=\\"%s\\"' % gain}
LOOP = [("V1", "dc 0 ac 1", ["ref", "GND"], None, None), ("E1", "100", ["e1", "GND", "ref", "fb"], None, E(100)), ("R1", "1k", ["e1", "o1"], None, None), ("C1", "100n", ["o1", "GND"], None, None),
        ("E2", "1", ["o2", "GND", "o1", "GND"], None, E(1)), ("R2", "1k", ["o2", "fb"], None, None), ("C2", "10n", ["fb", "GND"], None, None)]
FIXTURES = {"rc": RC, "div": DIV, "rlc": RLC, "fail": FAIL, "loop": LOOP}
LM358 = "* a stand-in follower for the e2e: out follows inp\n.subckt LM358 INP OUT GND\nRin INP GND 1G\nE1 OUT GND INP GND 1\n.ends\n"

def val(text):
    """A value as kicad_sim writes it ("1.58k") as a number."""
    mult = {"T": 1e12, "G": 1e9, "M": 1e6, "k": 1e3, "m": 1e-3, "u": 1e-6, "n": 1e-9, "p": 1e-12, "f": 1e-15}
    return float(text[:-1]) * mult[text[-1]] if text[-1] in mult else float(text)

class T:
    def __init__(self, m, sch): self.m, self.sch, self.bad, self.n, self.last = m, sch, 0, 0, None
    def sim(self, ops, **kw): self.last = self.m.call("kicad_sim", {"path": self.sch, "ops": ops, "show": False, **kw}); return self.last
    def err(self, ops):
        try: self.sim(ops)
        except RuntimeError as e: return str(e)
        return ""
    def ok(self, what, cond, got=""):
        self.n += 1; self.bad += 0 if cond else 1; print(("ok   " if cond else "FAIL ") + what + ("" if cond and not got else f": {json.dumps(got)[:1500]}"), flush=True)
    def near(self, what, got, want, rel=0.02, tol=0.0):
        good = isinstance(got, (int, float)) and abs(got - want) <= max(rel * abs(want), tol); self.ok(f"{what} = {want:.6g}", good, got)

def task_rc(t):
    r = t.sim([{"models": True}]); m = r["results"][0]["models"]; refs = sorted(p["ref"] for p in m["parts"])
    t.ok("models: the parts with no model, and why", refs == ["J1", "TP1", "U1"] and all("missing model" in p["why"] for p in m["parts"]), m)
    u1 = next(p for p in m["parts"] if p["ref"] == "U1"); t.ok("models: U1 with its MPN, nothing found here, the vendor rule", u1.get("mpn") == "LM358DR" and "found" not in u1 and "stop and ask the person" in m.get("next", ""), m)
    e = t.err([{"run": ".op"}]); r = None if e else t.sim([{"run": ".op"}])
    t.ok("a run with unmodelled parts fails in a line each", bool(r) and r["results"][0]["run"].get("failed") and any("U1" in p for p in r["results"][0]["run"]["problems"]), r or e)
    r = t.sim([{"exclude": "no_model"}, {"import": {"text": LM358, "url": "https://vendor.example/lm358.lib", "licence": "e2e stand-in, free to use"}, "to": "U1"},
               {"source": {"type": "pulse", "pos": "in", "params": {"y1": 0, "y2": 1, "td": 0, "tr": "1n", "tf": "1n", "tw": 1, "per": 2}, "ac": 1}},
               {"analysis": ".ac dec 20 1 1meg", "signals": ["V(out)"]}])
    res = r["results"]; imp = res[1]["import"]
    t.ok("exclude: the connector and the test point", res[0] == {"excluded": ["J1", "TP1"]}, res[0])
    t.ok("import: kept in sim/models with its source, assigned by pin names", imp["stored"] == "sim/models/lm358.lib" and imp["models"][0]["name"] == "LM358" and imp.get("pins_by") == "name" and imp["assigned"][0]["model"].upper() == "LM358", imp)
    note = json.load(open(os.path.dirname(t.sch) + "/sim/models/lm358.lib.source.json")); t.ok("import: the source note", note["url"] == "https://vendor.example/lm358.lib" and note["licence"].startswith("e2e") and len(note["sha256"]) == 64, note)
    t.ok("source: a pulse source V1 on in", res[2]["source"]["ref"] == "V1" and res[2]["source"]["neg"] == "GND" and "PULSE" in res[2]["source"].get("model", ""), res[2])
    t.ok("analysis: saved with its signals", res[3]["analysis"]["command"] == ".ac dec 20 1 1meg" and any("out" in s.lower() for s in res[3]["analysis"]["signals"]), res[3])
    t.ok("saved, and nothing left that would not simulate", r.get("saved") == t.sch and r.get("left") == [], {k: r.get(k) for k in ("saved", "left", "not_saved")})
    sch = open(t.sch).read(); t.ok("the schematic file has the model, the exclusions and the source", 'Sim.Library" "sim/models/lm358.lib"' in sch and sch.count("(exclude_from_sim yes)") == 2 and "Simulation_SPICE:VPULSE" in sch, "")
    t.ok("the source is out of the BOM and off the board", "(in_bom no)" in sch and "(on_board no)" in sch, "")
    fc = 1 / (2 * math.pi * 1e3 * 100e-9)
    r = t.sim([{"analysis": True}, {"run": True, "measures": {"fc": "corner V(out)", "dc": "dc_gain V(out)", "ph": f"phase_at V(out) x={fc}", "buf": "gain_at V(buf)/V(in) x=100", "ng": "ac WHEN vdb(/out)=-3"}}]); run = r["results"][1]["run"]
    t.ok("analysis: read back", r["results"][0]["analysis"]["tabs"][0]["command"] == ".ac dec 20 1 1meg", r["results"][0])
    t.near("run (saved .ac): -3 dB corner 1/(2 pi R C)", run["measures"]["fc"], fc); t.near("run: dc gain dB", run["measures"]["dc"], 0, tol=0.01); t.near("run: phase at the corner", run["measures"]["ph"], -45, rel=0.02)
    t.near("run: the imported model follows (V(buf)/V(in) at 100 Hz, dB)", run["measures"]["buf"], -10 * math.log10(1 + (100 / fc) ** 2), tol=0.02); t.near("run: ngspice's own meas", run["measures"]["ng"], fc, rel=0.02)
    t.ok("run: a line per signal, no vectors", run["signals"] and "waves" not in run and len(json.dumps(r)) < 1200, r)
    tau = 1e3 * 100e-9
    r = t.sim([{"run": ".tran 2u 2m", "signals": ["V(out)"], "waves": 24, "measures": {"tr": "rise_time V(out)", "ts": {"settling_time": "V(out)", "band": 0.02}, "os": "overshoot V(out)", "v1": f"at V(out) x={tau}", "fin": "final V(out)"}}]); run = r["results"][0]["run"]
    t.near("run (.tran): rise time tau ln 9", run["measures"]["tr"], tau * math.log(9), rel=0.03); t.near("run: settling to 2 %, tau ln 50", run["measures"]["ts"], tau * math.log(50), rel=0.03)
    t.near("run: value at tau", run["measures"]["v1"], 1 - math.exp(-1), rel=0.01); t.near("run: final", run["measures"]["fin"], 1, rel=0.01); t.ok("run: no overshoot", run["measures"]["os"] < 0.5, run["measures"])
    w = run["waves"]["V(out)"]; t.ok("run: waves as a few points", len(w["x"]) <= 24 and len(w["x"]) == len(w["y"]) and w["of"] > 100 and max(w["y"]) > 0.99, w)
    r = t.sim([{"sweep": "R1", "values": ["1k", "2k", "4.7k"], "run": ".ac dec 20 1 1meg", "measures": {"fc": "corner V(out)"}}]); sw = r["results"][0]["sweep"]
    t.ok("sweep: one table", sw["columns"] == ["R1", "fc"] and len(sw["rows"]) == 3 and "saved" not in r, sw)
    for row, rv in zip(sw["rows"], [1e3, 2e3, 4.7e3]): t.near(f"sweep: corner at R1 = {rv:g}", row[1], 1 / (2 * math.pi * rv * 100e-9))
    r = t.sim([{"sweep": "V1.y2", "values": [1, 2], "run": ".tran 2u 2m", "measures": {"fin": "final V(out)"}}]); t.ok("sweep: a source's parameter", [round(x[1], 2) for x in r["results"][0]["sweep"]["rows"]] == [1.0, 2.0], r)
    t.ok("sweep: the schematic is as it was", '"Value" "1k"' in open(t.sch).read(), "")
    r = t.sim([{"tune": "R1", "measure": "corner V(out)", "target": 1000, "from": "500", "to": "10k", "series": "E96", "write": True, "run": ".ac dec 20 1 1meg"}, {"run": True, "measures": {"fc": "corner V(out)"}}])
    tu = r["results"][0]["tune"]; t.ok("tune: 1.58k from E96, written", tu["value"] == "1.58k" and tu.get("written") is True and tu["runs"] <= 8 and '"Value" "1.58k"' in open(t.sch).read(), tu)
    t.near("tune: the corner with the written value", r["results"][1]["run"]["measures"]["fc"], 1 / (2 * math.pi * 1580 * 100e-9))
    r = t.sim([{"include": "TP1"}, {"models": "all"}, {"exclude": {"ref": "TP*"}}, {"model": "U1", "clear": True}, {"model": "U1", "lib": "sim/models/lm358.lib", "name": "LM358", "pins": {"1": "INP", "2": "OUT", "3": "GND"}}])
    parts = {p["ref"]: p for p in r["results"][1]["models"]["parts"]}
    t.ok("include, models all, exclude by pattern, model clear and by library with a pin map", r["results"][0] == {"included": ["TP1"]} and "why" in parts["TP1"] and parts["R1"].get("device", "").startswith("R") and parts["J1"].get("excluded") and r["results"][2] == {"excluded": ["TP1"]} and r["results"][4]["model"][0].get("pins") == "1=INP 2=OUT 3=GND" and r.get("left") == [], r)
    r = t.sim([{"probe": {"pin": "U1.2", "name": "BUFO"}}, {"run": ".op"}]); t.ok("probe: a pin's net labelled and read", "V(BUFO)" in r["results"][0]["probe"]["signals"] and '(label "BUFO"' in open(t.sch).read(), r["results"][0])
    r = t.sim([{"import": {"text": ".subckt REG VIN GND PH\nI1 VIN GND 1Adc\nD1 GND PH MBR340\n.ends\n", "url": "https://vendor.example/reg.lib", "as": "reg.lib"}}]); w = r["results"][0]["import"].get("warnings", [])
    t.ok("import: PSpice's ampere unit and an undefined model are warned of", len(w) == 2 and "atto" in w[0] and "mbr340" in w[1], w)
    e = t.err([{"import": {"text": "<html><body>Sign in to download</body></html>", "url": "https://vendor.example/login"}, "to": "U1"}]); t.ok("import: a login page is refused in words that say to ask the person", "stop and ask the person" in e, e)
    r = t.sim([{"stats": True}]); st = r["results"][0]["stats"]; t.ok("stats: the log's totals", st["actions"] >= 20 and st["ops"]["run"]["count"] >= 4 and st["ops"]["sweep"]["runs"] >= 5 and st["models_from"].get("vendor", 0) >= 1, st)

def task_div(t):
    r = t.sim([{"run": ".op", "signals": ["out"]}, {"run": ".op", "signals": ["V(out)"], "set": {"R2": "30k"}}, {"run": ".op"}]); res = r["results"]
    t.near("divider: V(out)", list(res[0]["run"]["values"].values())[0], 2.5, rel=1e-4); t.near("divider: with R2 = 30k for this run", list(res[1]["run"]["values"].values())[0], 3.75, rel=1e-4)
    t.ok("divider: all values when none is named, nothing saved", len(res[2]["run"]["values"]) >= 3 and "saved" not in r, res[2])
    r = t.sim([{"run": ".dc V1 0 5 0.5", "measures": {"v2": "at V(out) x=2", "top": "max V(out)", "i": "at I(R1) x=5"}}]); ms = r["results"][0]["run"]["measures"]
    t.near("dc sweep: V(out) at V1 = 2", ms["v2"], 1.0, rel=1e-4); t.near("dc sweep: the highest", ms["top"], 2.5, rel=1e-4); t.near("dc sweep: |I(R1)| at 5 V", abs(ms["i"]) if isinstance(ms["i"], (int, float)) else ms["i"], 5 / 20e3, rel=1e-3)
    r = t.sim([{"run": ".tf v(/out) V1"}]); t.ok("tf: its three numbers", any(abs(v - 0.5) < 1e-6 for v in r["results"][0]["run"].get("values", {}).values()), r["results"][0])
    k, temp = 1.380649e-23, 300.15; dens = math.sqrt(4 * k * temp * 5e3)
    r = t.sim([{"run": ".noise v(/out) V1 dec 10 1 1meg", "measures": {"n": "noise_rms onoise_spectrum from=10 to=10010", "d": "at onoise_spectrum x=1k"}}]); ms = r["results"][0]["run"]["measures"]
    t.near("noise: density of R1 || R2", ms["d"], dens, rel=0.02); t.near("noise: integrated over 10 kHz", ms["n"], dens * 100, rel=0.02)
    r = t.sim([{"run": ".noise v(/out) V1 dec 10 1 1meg", "set": {"V1": "dc 5"}}]); t.ok("noise: a source with no AC value is said so", "has no AC value" in json.dumps(r["results"][0]["run"].get("problems")), r["results"][0])
    r = t.sim([{"tune": "R2", "measure": "final V(out)", "target": 3.3, "from": "1k", "to": "100k", "series": "E24", "run": ".op"}]); tu = r["results"][0]["tune"]
    t.ok("tune: R2 for 3.3 V from E24, not written", tu["value"] == "20k" and "written" not in tu and '"Value" "10k"' in open(t.sch).read() and "saved" not in r, tu); t.near("tune: its result", tu["measure"], 5 * 20 / 30, rel=1e-3)
    e = t.err([{"tune": "R2", "measure": "final V(out)", "target": 9, "from": "1k", "to": "100k", "run": ".op"}]); t.ok("tune: a target out of reach says what the range gives", "is not between them" in e, e)
    e = t.err([{"run": ".op", "set": {"R2.nonsense": 1}}]); r = None if e else t.sim([{"run": ".op", "set": {"R2.nonsense": 1}}]); t.ok("an unknown override is a netlist error naming it", ("nonsense" in e) or (r and "nonsense" in json.dumps(r["results"][0]["run"].get("problems"))), e or r)

def task_rlc(t):
    r = t.sim([{"source": {"type": "pulse", "pos": "in", "params": {"y1": 0, "y2": 1, "td": 0, "tr": "1n", "tf": "1n", "tw": 1, "per": 2}, "ac": 1}}]); t.ok("source placed", r.get("saved") == t.sch, r)
    f0, bw, z = 1 / (2 * math.pi * math.sqrt(1e-3 * 100e-9)), 50 / (2 * math.pi * 1e-3), 25 * math.sqrt(100e-9 / 1e-3)
    r = t.sim([{"run": ".ac dec 100 100 10meg", "measures": {"bw": "bandwidth V(a)-V(c)", "fc": "center V(a)-V(c)", "q": "q V(a)-V(c)", "pk": {"peak": "V(c)"}, "lp": "bandwidth V(c) ref=dc"}}]); ms = r["results"][0]["run"]["measures"]
    t.near("rlc: band-pass bandwidth R/(2 pi L)", ms["bw"], bw); t.near("rlc: centre 1/(2 pi sqrt(LC))", ms["fc"], f0, rel=0.01); t.near("rlc: Q", ms["q"], f0 / bw)
    t.near("rlc: the low-pass peak, dB", ms["pk"]["db"] if isinstance(ms["pk"], dict) else ms["pk"], 20 * math.log10(1 / (2 * z * math.sqrt(1 - z * z))), rel=0.02)
    t.near("rlc: the low-pass -3 dB bandwidth", ms["lp"], f0 * math.sqrt(1 - 2 * z * z + math.sqrt(4 * z ** 4 - 4 * z * z + 2)))
    r = t.sim([{"run": ".tran 0.5u 2m 0 0.5u", "measures": {"os": "overshoot V(c) final=1", "fr": "frequency V(c) level=1 to=0.4m", "ts": "settling_time V(c) final=1", "fin": "final V(c)"}}]); ms = r["results"][0]["run"]["measures"]
    t.near("rlc: overshoot exp(-pi z / sqrt(1 - z^2)), percent", ms["os"], 100 * math.exp(-math.pi * z / math.sqrt(1 - z * z))); t.near("rlc: ringing frequency", ms["fr"], f0 * math.sqrt(1 - z * z)); t.near("rlc: final", ms["fin"], 1, rel=0.01)
    t.ok("rlc: settling within the envelope's bound", isinstance(ms["ts"], (int, float)) and 0.5 * math.log(50) / (z * 2 * math.pi * f0) < ms["ts"] <= 1.05 * math.log(50 / math.sqrt(1 - z * z)) / (z * 2 * math.pi * f0), ms["ts"])
    r = t.sim([{"sweep": "R1", "values": [20, 50, 100], "run": ".ac dec 100 100 10meg", "measures": {"q": "q V(a)-V(c)"}}]); rows = r["results"][0]["sweep"]["rows"]
    for row, rv in zip(rows, [20, 50, 100]): t.near(f"rlc sweep: Q at R1 = {rv}", row[1], 2 * math.pi * f0 * 1e-3 / rv, rel=0.03)
    r = t.sim([{"tune": "R1", "measure": "overshoot V(c) final=1", "target": 10, "from": 20, "to": 400, "run": ".tran 0.5u 2m 0 0.5u"}]); tu = r["results"][0]["tune"]
    zt = -math.log(0.1) / math.sqrt(math.pi ** 2 + math.log(0.1) ** 2); t.near("rlc tune: R1 for 10 % overshoot", val(tu["value"]), 2 * zt * math.sqrt(1e-3 / 100e-9), rel=0.03)

def task_fail(t):
    r = t.sim([{"run": ".op", "signals": ["mid"]}, {"run": ".op"}]); run = r["results"][0]["run"]
    t.ok("a floating node leads with the failure, without numbers", run.get("failed") and any("floats" in p or "singular" in p for p in run["problems"]) and "values" not in run and "measures" not in run, run)
    t.ok("problems are one line each, no console", all("\n" not in p and len(p) < 400 for p in run["problems"]) and "log" not in run and len(r["results"]) == 1 and "not run" in r.get("skipped", ""), r)
    r = t.sim([{"run": ".op", "log": True}]); t.ok("the console when asked", isinstance(r["results"][0]["run"].get("log"), str) and len(r["results"][0]["run"]["log"]) > 20, r["results"][0]["run"].get("log", "")[:80])
    e = t.err([{"run": ".plot v(mid)"}]); t.ok("what is not an analysis is said before KiCad is asked", "is not an analysis" in e, e)
    e = t.err([{"model": "R99", "device": "R"}]); t.ok("an unknown part", "R99" in e, e)
def task_loop(t):
    p1, p2 = 1 / (2 * math.pi * 1e3 * 100e-9), 1 / (2 * math.pi * 1e3 * 10e-9); mag = lambda f: 100 / math.sqrt((1 + (f / p1) ** 2) * (1 + (f / p2) ** 2))
    lo, hi = p1, 1e7
    for _ in range(200): mid = math.sqrt(lo * hi); lo, hi = (mid, hi) if mag(mid) > 1 else (lo, mid)
    fc = math.sqrt(lo * hi); pm = 180 - math.degrees(math.atan(fc / p1)) - math.degrees(math.atan(fc / p2))
    # at the amplifier's input (nothing flows into it), and at its output (everything does): the same loop, the circuit's own AC source taken out
    for at in ("E1.4", "R1.1"):
        r = t.sim([{"loopgain": at}]); l = r["results"][0]["loopgain"]
        t.near(f"loop gain cut at {at}: crossover", l["crossover"], fc, rel=0.01); t.near(f"loop gain cut at {at}: phase margin", l["phase_margin"], pm, rel=0.01); t.near(f"loop gain cut at {at}: DC loop gain, dB", l["dc_gain_db"], 40, rel=0.002)
        t.ok(f"loop gain cut at {at}: two poles have no gain margin, three runs, nothing saved, no curve", str(l["gain_margin"]).startswith("none:") and l["runs"] == 3 and "curve" not in l and "saved" not in r, l)
    r = t.sim([{"loopgain": "E1.4", "set": {"C2": "100n"}, "waves": 16, "run": ".ac dec 100 1 10meg"}]); l = r["results"][0]["loopgain"]
    t.near("loop gain with C2 = 100n for these runs: both poles at one frequency", l["phase_margin"], 180 - 2 * math.degrees(math.atan(math.sqrt(99))), rel=0.02)
    t.ok("loop gain: the curve when asked", len(l["curve"]["f"]) <= 16 and abs(l["curve"]["db"][0] - 40) < 0.1, l.get("curve"))
    t.ok("the schematic is untouched", "Vlg" not in open(t.sch).read() and '"Value" "10n"' in open(t.sch).read(), "")
    e = t.err([{"loopgain": "E9.1"}]); t.ok("a cut at a part that is not there", "E9" in e, e)
    r = t.sim([{"run": ".ac dec 20 1 1meg", "options": {"save_all_currents": False}, "directives": [".options reltol=0.002"], "measures": {"cl": "dc_gain V(fb)", "bw": "corner V(fb)"}}]); ms = r["results"][0]["run"]["measures"]
    t.near("closed loop with netlist options and directives: gain 100/101, dB", ms["cl"], 20 * math.log10(100 / 101), tol=0.01)
# Three projects at once and room for two headless KiCads: one runs a long transient (tens of seconds of ngspice, few points kept),
# the other two keep asking for operating points. The long run ends with its numbers and no call of the others fails: a server at
# work is never stopped to make room, so one of the two is served beside the run and the other waits for room.
SLOW_S = 40
def task_slow(t):
    t0 = time.time(); r = t.sim([{"run": f".tran 0.1 {SLOW_S} 0 1u", "directives": [".options interp"], "signals": ["out"], "measures": {"end": "final V(out)"}, "retry": False, "timeout": 300, "max_run_points": 400_000_000}]); run = r["results"][0]["run"]
    t.ok("the long transient ends with its numbers while two other projects are served", "failed" not in run and abs(run["measures"]["end"] - 5) < 0.01, run)
    t.ok("it was long enough to be in the others' way (10 s or more)", time.time() - t0 >= 10, round(time.time() - t0, 1))
def task_busy(t):
    t0 = time.time(); worst, n, bad = 0.0, 0, []
    while time.time() - t0 < SLOW_S + 5:
        c0 = time.time(); e = t.err([{"run": ".op", "signals": ["out"]}]); worst = max(worst, time.time() - c0); n += 1
        if e or abs(t.last["results"][0]["run"]["values"]["V(/out)"] - 2.5) > 1e-6: bad.append(e or t.last)
        time.sleep(0.5)
    # (room for two: the third project's first call waits until one of the others has been idle a while, here until the long run ends)
    t.ok(f"every one of {n} operating points beside the long run was answered", not bad and n >= 1, bad[:3])
    t.ok("none waited past the long run and the room wait", worst < SLOW_S + 45, round(worst, 1)); print(f"note {n} calls, the longest {worst:.1f} s")
# A transient of two million points: KiCad thins what the hub reads to 200,000 a vector, the answer says so, the number is still right,
# and the hub's memory high-water mark moves by less than BIG_MB (docs/kicad-sim.md "Memory"); with tstart only the window is kept at all.
BIG_MB = 150
def hub_peak_mb(t):
    pid = json.load(open(os.path.dirname(os.path.dirname(os.path.dirname(t.sch))) + "/hub.json"))["pid"]
    return int(next(l for l in open(f"/proc/{pid}/status") if l.startswith("VmHWM")).split()[1]) / 1024
def task_big(t):
    t.sim([{"run": ".op"}]); before = hub_peak_mb(t)
    r = t.sim([{"run": ".tran 1u 2 0 1u", "signals": ["out", "in"], "measures": {"end": "final V(out)", "top": "max V(out)"}, "timeout": 300, "max_run_points": 20_000_000}]); run = r["results"][0]["run"]; grew = hub_peak_mb(t) - before
    t.ok("two million points: the answer has its numbers", "failed" not in run and abs(run["measures"]["end"] - 5) < 0.01 and run["points"] >= 2_000_000, {k: run.get(k) for k in ("failed", "problems", "measures", "points", "ms")})
    t.ok("it says the vectors were thinned, and how", "of 2000" in run.get("thinned", "") and "tstart" in run.get("thinned", "") and "lowest and highest" in run.get("thinned", ""), run.get("thinned"))
    lim = t.sim([{"run": ".tran 1u 2 0 1u", "signals": ["out", "in"], "measures": {"end": "final V(out)"}, "retry": False, "timeout": 300}])["results"][0]["run"]
    t.ok("without max_run_points KiCad stops the run at 5,000,000 values: a failure that says so and what to do, no numbers", lim.get("failed") == "the run reached its point limit" and "measures" not in lim and "max_run_points" in json.dumps(lim.get("problems")), {k: lim.get(k) for k in ("failed", "problems", "measures")})
    t.ok(f"the hub's memory peak grew by less than {BIG_MB} MB", grew < BIG_MB, round(grew, 1)); print(f"note hub peak grew {grew:.1f} MB for 2,000,000 points thinned to 200,000")
    r = t.sim([{"run": ".tran 1u 2 0 1u", "tstart": "1.99", "signals": ["out"], "measures": {"end": "final V(out)"}, "timeout": 300}]); run = r["results"][0]["run"]
    t.ok("with tstart only the window is kept: 10,000 points, none thinned", "failed" not in run and 9_000 <= run["points"] <= 11_000 and "thinned" not in run and abs(run["measures"]["end"] - 5) < 0.01, {k: run.get(k) for k in ("failed", "problems", "points", "thinned")})
    e = t.err([{"run": ".tran 1n 100 0 1n"}]); t.ok("a line that says a hundred billion points is refused before ngspice", "points or more" in e, e)
# A turn interrupted while its run is out: the hub drops the call and stops the run in KiCad. The first turn starts a transient of
# minutes; the runner interrupts it; a second turn in the same project finds the simulator free (a run still going would refuse it).
def task_stop(t):
    mark = os.path.dirname(t.sch) + "/first-turn"
    if not os.path.exists(mark):
        open(mark, "w").write("x"); t.sim([{"run": ".tran 0.1 400 0 1u", "directives": [".options interp"], "signals": ["out"], "retry": False, "timeout": 600, "max_run_points": 1_000_000_000}])
        t.ok("the first turn was interrupted before its run ended", False, "the run of minutes came back"); return
    t0 = time.time(); e = t.err([{"run": ".op", "signals": ["out"]}])
    t.ok("after the interrupt the simulator is free: an operating point is answered at once", not e and abs(t.last["results"][0]["run"]["values"]["V(/out)"] - 5) < 1e-6 and time.time() - t0 < 10, e or t.last)
SLOW = [("V1", "dc 5", ["in", "GND"], None, None), ("R1", "1k", ["in", "out"], None, None), ("C1", "1u", ["out", "GND"], None, None)]
FIXTURES.update({"slow": SLOW, "busya": DIV, "busyb": DIV, "big": SLOW, "stop": SLOW})
CROWD = ["slow", "busya", "busyb"]
TASKS = {"rc": task_rc, "div": task_div, "rlc": task_rlc, "fail": task_fail, "loop": task_loop, "slow": task_slow, "busya": task_busy, "busyb": task_busy, "big": task_big, "stop": task_stop}

def run(work, only):
    fix = f"{work}/simfix"; shutil.rmtree(fix, ignore_errors=True); bad = 0
    for name, parts in FIXTURES.items(): write_project(f"{fix}/{name}", name, parts)
    log = f"{json.load(open(work + '/hub.json'))['home']}/logs/sim.jsonl"
    # (the crowd's three turns run at once, the busy ones started a moment after the long one; everything else one after another)
    names = only or [n for n in TASKS if n not in CROWD]; turns = {} # (the crowd is asked for by name, `run WORK crowd`: it needs the pool fix of #1182, without which the long run's server is stopped to make room)
    def start(name):
        d = f"{work}/runs/e2e-{name}"; shutil.rmtree(d, ignore_errors=True); shutil.copytree(f"{fix}/{name}", d)
        return subprocess.Popen(["bun", HERE + "/turn.mjs", work, d, f"ORACLE {os.path.abspath(__file__)}"], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env={**os.environ, "TURN_MS": "600000"})
    for name in [x for n in names for x in (CROWD if n == "crowd" else [n])]:
        before = len(open(log).read().splitlines()) if os.path.exists(log) else 0
        if name == "stop":  # the first turn is left running, interrupted after 10 s, and the second turn is the one judged
            d = f"{work}/runs/e2e-stop"; shutil.rmtree(d, ignore_errors=True); shutil.copytree(f"{fix}/stop", d)
            first = json.loads(subprocess.run(["bun", HERE + "/turn.mjs", work, d, f"ORACLE {os.path.abspath(__file__)}", "nowait"], capture_output=True, text=True).stdout.strip().splitlines()[-1])
            time.sleep(10); cut = subprocess.run(["bun", HERE + "/interrupt.mjs", work, first["thread"]], capture_output=True, text=True).stdout.strip()
            time.sleep(4); print(f"stop: the first turn after the interrupt: {cut}", flush=True)
            if os.path.exists(f"{work}/out/e2e-stop.oracle.log"): os.remove(f"{work}/out/e2e-stop.oracle.log")
            out0 = subprocess.run(["bun", HERE + "/turn.mjs", work, d, f"ORACLE {os.path.abspath(__file__)}"], capture_output=True, text=True, env={**os.environ, "TURN_MS": "600000"}).stdout
        elif name in CROWD:
            if not turns:
                for c in CROWD: turns[c] = start(c); time.sleep(3)
            out0 = turns[name].communicate()[0]
        else: out0 = start(name).communicate()[0]
        r = type("R", (), {"stdout": out0, "stderr": ""})
        out = open(f"{work}/out/e2e-{name}.oracle.log").read() if os.path.exists(f"{work}/out/e2e-{name}.oracle.log") else r.stdout + r.stderr
        fails = [l for l in out.splitlines() if l.startswith("FAIL")]; done = [l for l in out.splitlines() if l.startswith("done ")]
        lines = []
        for _ in range(60):  # the turn's own line follows its end by a moment (the hub waits for the provider's usage)
            lines = []
            for l in (open(log).read().splitlines()[before:] if os.path.exists(log) else []):
                try: lines.append(json.loads(l))
                except ValueError: pass  # (a line another turn is writing right now)
            if any(l.get("kind") == "turn" for l in lines): break
            time.sleep(0.1)
        acts, turn = [l for l in lines if l.get("kind") != "turn"], [l for l in lines if l.get("kind") == "turn"]
        logged = bool(acts) and all(k in acts[0] for k in ("at", "thread", "turn", "op", "hub_ms", "bytes")) and any("timings" in a and "ngspice_ms" in a for a in acts) and len(turn) == 1 or name in CROWD or name == "stop"  # (the crowd's lines interleave: not counted per task)
        ok = not fails and bool(done) and logged; bad += 0 if ok else 1
        print(f"{name}: {'PASS' if ok else 'FAIL'} {done[-1] if done else '(the task did not finish)'}; log: {len(acts)} actions, {sum(a.get('runs', 0) for a in acts)} runs, turn line {'yes' if turn else 'NO'}", flush=True)
        for l in fails: print("   ", l[:400], flush=True)
        if not done: print("   ", out[-1500:], flush=True)
    return bad

if __name__ == "__main__":
    if sys.argv[1] == "run": sys.exit(1 if run(sys.argv[2], sys.argv[3:]) else 0)
    from mcp import Mcp
    cwd = sys.argv[2]; name = os.path.basename(cwd).split("-", 1)[1]; t = T(Mcp(sys.argv[1]), f"{cwd}/{name}.kicad_sch")
    try: TASKS[name](t)
    except Exception as e: t.ok(f"the task ran to its end ({type(e).__name__})", False, str(e)[:600] + " | last answer: " + json.dumps(t.last)[:4000])
    print(f"done {t.n - t.bad}/{t.n} checks, {t.m.calls} calls, {t.m.bytes} bytes answered")
