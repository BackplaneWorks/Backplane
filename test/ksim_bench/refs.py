"""The benchmark's reference answers, each derived twice (docs/kicad-sim-bench.md):
  derived(task)              from the component values alone: a formula, or the circuit's equations solved by mna.py (no KiCad, no ngspice)
  simulated(task, rt, dir)   by ngspice (the runtime's libngspice, tools/ngs.py) on the SPICE netlist KiCad's own exporter writes from the solved
                             fixture (mkfix.py's <fixture>.solved): the fixture, its Sim.* fields and the exporter are all in the path
A regulator and a converter have no closed form: their reference is the simulation, and `derived` gives the hand estimate it is held against.
refs.py RUNTIME FIXTURES [task ...] prints both for every task."""
import cmath, json, math, os, re, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE); sys.path.insert(0, HERE + "/tools")
import mna, ngs
from mna import Ckt, db

KT4 = 4 * 1.380649e-23 * 300.15  # 4kT at ngspice's default 27 C
E24 = [1.0, 1.1, 1.2, 1.3, 1.5, 1.6, 1.8, 2.0, 2.2, 2.4, 2.7, 3.0, 3.3, 3.6, 3.9, 4.3, 4.7, 5.1, 5.6, 6.2, 6.8, 7.5, 8.2, 9.1]

def export(cli, d):
    """the SPICE netlist of the project in d, as KiCad writes it (run in d: a relative Sim.Library is looked for from the working directory first)"""
    sch = next(f for f in sorted(os.listdir(d)) if f.endswith(".kicad_sch")); out = os.path.join(d, ".ksim-netlist.cir")
    r = subprocess.run([cli, "sch", "export", "netlist", "--format", "spice", "-o", out, sch], capture_output=True, text=True, cwd=d)
    try: t = open(out, errors="replace").read(); os.remove(out)
    except OSError: t = ""
    return t, (r.stdout + r.stderr).strip()
def edit(deck, subs, add=()):
    """the deck with lines rewritten ([(regex, replacement)], each must match) and lines added before .end"""
    for pat, rep in subs:
        deck, k = re.subn(pat, rep, deck, flags=re.M | re.I)
        if not k: raise ValueError("netlist line not found: " + pat)
    return deck.replace("\n.end", "\n" + "\n".join(add) + "\n.end", 1) if add else deck
def run(rt, deck, cmds, vecs, d, compat="", timeout=3000):
    o = ngs.run(rt, deck, cmds, vecs, compat, cwd=d, timeout=timeout)
    if any(v is None for v in o["vecs"].values()) or not o["ok"]: raise RuntimeError("ngspice: " + "; ".join(o["problems"][:4]) + " | " + o["log"][-400:])
    return {k: [x[0] + 1j * x[1] if isinstance(x, list) else x for x in v] for k, v in o["vecs"].items()}
def ac(rt, deck, d, f0, f1, per_dec, *nodes):
    v = run(rt, deck, [f"ac dec {per_dec} {f0} {f1}"], ["frequency"] + list(nodes), d)
    return [x.real for x in v["frequency"]], [v[n] for n in nodes]
def corner(fs, H):
    """-3 dB frequency relative to the low-frequency gain (the last downward crossing), and the peak above that gain in dB"""
    g = [db(h) for h in H]; return mna.cross(fs, g, g[0] - 10 * math.log10(2), last=True), max(g) - g[0]

# ---- lcfilt: L1 into C1 || (R1 + C2) || the 8 ohm load
def lc_H(f, L=22e-6, C1=680e-9, R1=15.0, C2=220e-9, RL=8.0):
    s = 2j * math.pi * f; Z = 1 / (1 / RL + s * C1 + 1 / (R1 + 1 / (s * C2))); return Z / (Z + s * L)
def lc_corner(sim=None):
    if sim: fs, (vo, vi) = ac(*sim, 100, 1e6, 400, "/out", "/in"); H = [a / b for a, b in zip(vo, vi)]
    else: fs = mna.logspace(100, 1e6, 2000); H = [lc_H(f) for f in fs]
    f3, pk = corner(fs, H); return {"f3db_hz": f3, "peak_db": pk}

# ---- preamp: the BP321 macro-model (models/BP321.lib) in its circuit
def preamp(loop=False, noise=None):
    """the stage's equations. loop: the op-amp's transconductance is taken out and driven by hand (source "it"), for the return ratio;
    noise: the name of the one noise source that is on"""
    c = Ckt(); c.V("vin", "in", "0"); c.C("in", "inp", 1e-6)
    for name, a, b, r in (("R1", "vcc", "inp", 100e3), ("R2", "inp", "0", 100e3), ("R3", "aout", "inn", 12e3), ("R4", "inn", "fbg", 10e3)):
        c.R(a, b, r); c.I(name, a, b)  # each resistor's thermal noise current, in parallel with it
    c.V("vcc", "vcc", "0"); c.C("fbg", "0", 10e-6); c.C("vcc", "0", 100e-9); c.C("aout", "0", 4.7e-9)
    c.V("en", "inx", "inp")                                             # the op-amp: its input noise voltage in series with the + input
    c.R("vcc", "mid", 1e6); c.R("mid", "0", 1e6); c.R("inx", "mid", 1e12); c.R("inn", "mid", 1e12)
    if loop: c.I("it", "0", "n1")
    else: c.G("0", "n1", "inx", "inn", 1e-3)
    c.R("n1", "0", 100e6); c.C("n1", "0", 159.155e-12); c.E("n2", "0", "n1", "0", 1); c.R("n2", "n3", 1e3); c.C("n3", "0", 53.0516e-12)
    c.E("n4", "n5", "n3", "0", 1); c.E("n5", "0", "mid", "0", 1); c.R("n4", "aout", 100)
    return c
def opamp_gain_pm(sim=None):
    if sim:
        rt, deck, d = sim; f, (vo, vi) = ac(rt, deck, d, 1e3, 1e3 * 1.0001, 1, "/aout", "/in"); gain = abs(vo[0] / vi[0])
        # the loop opened at the op-amp's inverting input (nothing flows there): 1 GH keeps the DC path, 1 GF brings the test signal in
        brk = edit(deck, [(r"^(XU1\s+\S+\s+)/INN(\s)", r"\1inn_x\2"), (r"^(V1 .*) AC 1\b", r"\1 AC 0")], ["Lbrk /INN inn_x 1e9", "Cbrk inn_x vinj 1e9", "Vinj vinj 0 dc 0 ac 1"])
        fs, (a, b) = ac(rt, brk, d, 10, 100e6, 200, "/inn", "inn_x"); T = [-x / y for x, y in zip(a, b)]
    else:
        gain = abs(preamp().solve(1e3, {"vin": 1})["aout"]); c = preamp(loop=True); fs = mna.logspace(10, 100e6, 400); T = []
        for f in fs: v = c.solve(f, {"it": 1e-3}); T.append(-(v["inx"] - v["inn"]))
    pm, fc = mna.margin(fs, T); return {"gain_vv": gain, "pm_deg": pm, "_crossover_hz": fc}
def noise(sim=None, f0=20.0, f1=20e3):
    if sim:
        rt, deck, d = sim
        v = run(rt, deck, [f"noise v(/aout) v1 dec 200 {f0} {f1}", "setplot noise2", "let tot = onoise_total"], ["tot"], d); return {"noise_uvrms": abs(v["tot"][0]) * 1e6}
    c = preamp(); fs = mna.logspace(f0, f1, 300); psd = []
    for f in fs:
        s = 8e-9 ** 2 * abs(c.solve(f, {"en": 1})["aout"]) ** 2
        for name, r in (("R1", 100e3), ("R2", 100e3), ("R3", 12e3), ("R4", 10e3)): s += KT4 / r * abs(c.solve(f, {name: 1})["aout"]) ** 2
        psd.append(s)
    tot = sum((psd[i] + psd[i + 1]) / 2 * (fs[i + 1] - fs[i]) for i in range(len(fs) - 1)); return {"noise_uvrms": math.sqrt(tot) * 1e6}

# ---- ldo: BP758 (models/BP758.lib), 5 V in, 3.3 V out, a load step from 10 mA to 200 mA in 1 us
def ldo_small_signal(iload=0.2):
    """the loop linearised by hand at the load: the pass transistor's gm and gds from its level-1 equations (KP 1.6, VTO -0.7, LAMBDA 0.02)"""
    vsd = 5 - 3.3; i = iload + 3.3 / 120e3; vov = math.sqrt(2 * i / (1.6 * (1 + 0.02 * vsd))); gm = 2 * i / vov; gds = 0.02 * i / (1 + 0.02 * vsd)
    c = Ckt(); c.I("it", "g", "0"); c.R("g", "0", 3e6); c.R("g", "gz", 120e3); c.C("gz", "0", 20e-12); c.C("g", "0", 2e-12)          # the error amplifier's output current, driven by hand
    c.G("0", "out", "0", "g", gm); c.R("out", "0", 1 / gds); c.C("out", "0", 10e-6); c.R("out", "fb", 100e3); c.R("fb", "0", 20e3)
    fs = mna.logspace(1, 100e6, 300); T = [c.solve(f, {"it": 100e-6})["fb"] for f in fs]    # Gea pulls 100 uS * (ref - fb) out of the gate
    return mna.margin(fs, T)
def ldo_step(t, y, t0=2e-3):
    """the load step's numbers from the output's waveform: the dip below the value before the step, and when it last leaves 5 mV around its new value"""
    pre = [b for a, b in zip(t, y) if a < t0][-1]; after = [(a, b) for a, b in zip(t, y) if a >= t0]
    final = [b for a, b in after if a > t0 + 1.8e-3]; final = sum(final) / len(final); late = [a for a, b in after if abs(b - final) > 5e-3]
    return {"undershoot_mv": (pre - min(b for _, b in after)) * 1e3, "recover_us": (late[-1] - t0) * 1e6, "_pre_v": pre, "_final_v": final}
def ldo(sim=None):
    if not sim: pm, fc = ldo_small_signal(); return {"pm_deg": pm, "_crossover_hz": fc, "_note": "hand estimate: the loop linearised at 200 mA; the transient has no closed form"}
    rt, deck, d = sim
    v = run(rt, deck, ["tran 50n 4m 1.9m 50n"], ["time", "/vout"], d); t, y = [x.real for x in v["time"]], [x.real for x in v["/vout"]]
    out = ldo_step(t, y)
    brk = edit(deck, [(r"^(XU1\s+\S+\s+\S+\s+\S+\s+)/FB(\s)", r"\1fb_x\2"), (r"^I1 (\S+ \S+) .*$", r"I1 \1 DC 200m")], ["Lbrk /FB fb_x 1e9", "Cbrk fb_x vinj 1e9", "Vinj vinj 0 dc 0 ac 1"])
    fs, (a, b) = ac(rt, brk, d, 1, 100e6, 200, "/fb", "fb_x"); out["pm_deg"], out["_crossover_hz"] = mna.margin(fs, [-x / y for x, y in zip(a, b)])
    return out

# ---- buck: TPS54331 with the vendor's model, 12 V in, 3.3 V out, a 2 A load, 220 uF with 60 mohm
BUCK_OPTS = ".options method=gear reltol=0.01 abstol=1e-7 vntol=1e-4 itl4=500"  # the vendor's PSpice model does not get through ngspice's defaults
def buck(sim=None, step="100n", tstop=8e-3, opts=BUCK_OPTS):
    if not sim:  # the textbook estimate: the inductor's ripple current through the capacitor's series resistance, plus the capacitor's own charge ripple
        f, L, esr, C, vf, rds = 570e3, 6.8e-6, 60e-3, 220e-6, 0.4, 0.08; duty = (3.3 + vf) / (12 - 2 * rds + vf); dil = (3.3 + vf) * (1 - duty) / (L * f)
        return {"ripple_mv": (dil * esr + dil / (8 * f * C)) * 1e3, "_il_pp_a": dil, "_note": "hand estimate at the datasheet's 570 kHz: ripple current times ESR, plus the capacitor's charge ripple"}
    rt, deck, d = sim; deck = edit(deck, [], [opts]) if opts else deck
    v = run(rt, deck, [f"tran {step} {tstop}"], ["time", "/vout", "/sw"], d, compat="psa"); t, y, sw = ([x.real for x in v[k]] for k in ("time", "/vout", "/sw"))
    win = [(a, b, c) for a, b, c in zip(t, y, sw) if a >= tstop - 1e-3]; ys = [b for _, b, _ in win]
    rises = [win[i][0] for i in range(len(win) - 1) if win[i][2] < 6 <= win[i + 1][2]]
    return {"ripple_mv": (max(ys) - min(ys)) * 1e3, "_vout_v": sum(ys) / len(ys), "_fsw_hz": (len(rises) - 1) / (rises[-1] - rises[0]) if len(rises) > 1 else None}

# ---- vsense: R1 from the bus, R2 to ground, R3 to 3.3 V, each at the end of its tolerance
def vsense(sim=None, vbus=24.0, v33=3.3):
    nom = {"R1": (180e3, 0.01), "R2": (22e3, 0.01), "R3": (220e3, 0.05)}; vals = []
    for k in range(8):
        r = {n: v * (1 + t * (1 if k >> i & 1 else -1)) for i, (n, (v, t)) in enumerate(nom.items())}
        if sim:
            rt, deck, d = sim; dk = edit(deck, [(rf"^({n} \S+ \S+) \S+", rf"\1 {x:.6f}") for n, x in r.items()])
            vals.append(run(rt, dk, ["op"], ["/adc"], d)["/adc"][0].real)
        else: vals.append((vbus / r["R1"] + v33 / r["R3"]) / (1 / r["R1"] + 1 / r["R2"] + 1 / r["R3"]))
    return {"vmin_v": min(vals), "vmax_v": max(vals)}

# ---- aafilter: the E24 value of C2 that puts the corner nearest 4 kHz
def aa_H(f, c2, R1=2.2e3, C1=10e-9, R2=4.7e3, R3=100e3):
    s = 2j * math.pi * f; z2 = 1 / (1 / R3 + s * c2); zm = 1 / (s * C1 + 1 / (R2 + z2)); return zm / (R1 + zm) * z2 / (R2 + z2)
def aa_f3(c2): fs = mna.logspace(10, 10e6, 100); return corner(fs, [aa_H(f, c2) for f in fs])[0]
def aa_exact(c2):
    lvl = db(aa_H(1e-3, c2)) - 10 * math.log10(2); f = aa_f3(c2); return mna.refine(lambda x: -db(aa_H(x, c2)), f / 1.2, f * 1.2, -lvl)
def tune(sim=None, target=4000.0):
    cand = sorted(((abs(aa_exact(m * 10 ** e) - target), m * 10 ** e) for e in (-10, -9, -8, -7) for m in E24)); best = cand[0][1]
    if sim:
        rt, deck, d = sim; out = {}
        for c in (best, cand[1][1]):
            fs, (vo, vi) = ac(rt, edit(deck, [(r"^(C2 \S+ \S+) \S+", rf"\1 {c:.6e}")]), d, 10, 1e6, 400, "/out", "/in"); out[c] = corner(fs, [a / b for a, b in zip(vo, vi)])[0]
        return {"c2_nf": best * 1e9, "f3db_hz": out[best], "_next_best_nf": cand[1][1] * 1e9, "_next_best_f3db_hz": out[cand[1][1]]}
    return {"c2_nf": best * 1e9, "f3db_hz": aa_exact(best), "_next_best_nf": cand[1][1] * 1e9, "_next_best_f3db_hz": aa_exact(cand[1][1])}

TASKS = {"lc_corner": ("lcfilt", lc_corner), "opamp_gain_pm": ("preamp", opamp_gain_pm), "ldo_transient": ("ldo", ldo), "buck_ripple": ("buck", buck),
         "divider_tol": ("vsense", vsense), "tune_corner": ("aafilter", tune), "noise": ("preamp", noise)}
def derived(task): return TASKS[task][1]()
def simulated(task, rt, fixtures):
    fx, fn = TASKS[task]; d = f"{fixtures}/{fx}.solved"; deck, err = export(rt + "/bin/kicad-cli", d)
    if not deck: raise RuntimeError(f"{d}: no netlist: {err}")
    return fn((rt, deck, d))
if __name__ == "__main__":
    rt, fx = sys.argv[1:3]
    for t in sys.argv[3:] or TASKS:
        print(t, "derived  ", json.dumps(derived(t)), flush=True); print(t, "simulated", json.dumps(simulated(t, rt, fx)), flush=True)
