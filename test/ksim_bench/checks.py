"""The simulation benchmark's pass check (tasks.json): check(task, run_dir, fixtures_dir, cli, answer_text) -> [what is wrong]; empty is a pass.
Two halves, both needed:
  the answer   the last `RESULT key=<number> ...` line of the agent's final message, each number within the task's tolerance of the reference
  the design   the saved schematic is still the board's schematic and now simulates: every part the fixture had is there with its value, its
               BOM/board flags and its connections; what was added is out of the BOM and off the board; KiCad's own SPICE netlist of it has
               no part without a model, leaves out the parts that do not simulate, names the model the task asks for on the right pins, has a
               source, and loads (and, for a linear circuit, finds its operating point) in ngspice; a tuned value is written in the schematic
A task whose model cannot be had without a sign-in passes the other way round: no number, the reason said, and no model put in its place.
Written before any run."""
import json, os, re, sys
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE); sys.path.insert(0, HERE + "/tools"); sys.path.insert(0, os.path.join(HERE, "..", "kipc_bench"))
import kx, norm, ngs, refs

NUM = r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?"
def result(text):
    """{key: number} of the last RESULT line (a key whose value is not a plain number is left out), or None when there is no such line"""
    lines = [l for l in (text or "").splitlines() if re.search(r"\bRESULT\b", l)]
    if not lines: return None
    return {k.lower(): float(v) for k, v in re.findall(rf"([A-Za-z_][A-Za-z0-9_]*)\s*=\s*`?({NUM})(?![A-Za-z0-9.])", lines[-1].replace("*", ""))}
def answer(spec, text):
    got = result(text); bad = []
    if got is None: return ["no RESULT line in the answer"]
    for k, (ref, tol, kind) in spec.items():
        if k not in got: bad.append(f"{k}: no number in the RESULT line"); continue
        lim = tol if kind == "abs" else abs(ref) * tol
        if abs(got[k] - ref) > lim: bad.append(f"{k}: {got[k]:g}, reference {ref:g} +/- {lim:.3g}")
    return bad

SI = {"f": 1e-15, "p": 1e-12, "n": 1e-9, "u": 1e-6, "µ": 1e-6, "μ": 1e-6, "m": 1e-3, "k": 1e3, "meg": 1e6, "M": 1e6, "g": 1e9, "G": 1e9, "r": 1, "R": 1}
def si(s):
    """a component value as a number: 3.9n, 3n9, 3900p, 3.9 nF, 0.0039u, 3.9e-9, 4k7, 2.2k"""
    s = s.strip().replace(" ", ""); m = re.fullmatch(rf"({NUM})(meg|[fpnuµμmkMgGrR])?[A-Za-zΩ]*", s)
    if m: return float(m.group(1)) * SI.get(m.group(2) or "r", 1)
    m = re.fullmatch(r"(\d+)(meg|[pnuµμmkMrR])(\d+)[A-Za-zΩ]*", s)  # 4k7
    return float(m.group(1) + "." + m.group(3)) * SI[m.group(2)] if m else None

def parts(sch):
    """{reference: {"lib", "value", "flags", "props"}} of a schematic file (power symbols left out)"""
    t = norm.parse(open(sch).read()); out = {}
    for s in norm.kids(t, "symbol"):
        p = norm.props(s); r = p.get("Reference", "?")
        if r.startswith("#"): continue
        out[r] = {"lib": norm.val(s, "lib_id"), "value": p.get("Value", ""), "props": p, "flags": {k: norm.val(s, k, d="no") for k in ("exclude_from_sim", "in_bom", "on_board", "dnp")}}
    return out
def groups(cli, sch, refs_):
    """how the pins of the given parts are joined: a set of pin sets, whatever the nets are called"""
    _, nets = kx.netlist(cli, sch); g = set()
    for pins in nets.values():
        s = frozenset((r, p) for r, p in pins if r in refs_)
        if len(s) > 1: g.add(s)
    return g
def lines(deck):
    out = []
    for l in deck.splitlines():
        if l.startswith("+") and out: out[-1] += " " + l[1:]
        elif l.strip() and not l.startswith("*"): out.append(l.strip())
    return {l.split()[0].upper(): l.split() for l in out if not l.startswith(".")}
def sch_of(d): return os.path.join(d, next(f for f in sorted(os.listdir(d)) if f.endswith(".kicad_sch")))

def design(spec, d, fx, cli):
    bad = []; was = parts(sch_of(fx)); now = parts(sch_of(d)); tuned = spec.get("tuned", {})
    for r, p in was.items():
        if r not in now: bad.append(f"{r} is gone"); continue
        q = now[r]
        if q["lib"] != p["lib"]: bad.append(f"{r}: symbol {q['lib']} (was {p['lib']})")
        if r in tuned:
            v = si(q["value"])
            if v is None or abs(v - tuned[r]) > 0.005 * tuned[r]: bad.append(f"{r}: value {q['value']!r} in the schematic, reference {tuned[r]:g}")
        elif q["value"] != p["value"]: bad.append(f"{r}: value {q['value']!r} (was {p['value']!r})")
        for k in ("in_bom", "on_board", "dnp"):
            if q["flags"][k] != p["flags"][k]: bad.append(f"{r}: {k} {q['flags'][k]} (was {p['flags'][k]})")
    for r, q in now.items():
        if r not in was and (q["flags"]["in_bom"] != "no" or q["flags"]["on_board"] != "no"): bad.append(f"{r} was added for simulation but is in the BOM or on the board")
    if groups(cli, sch_of(d), set(was)) != groups(cli, sch_of(fx), set(was)): bad.append("the circuit's connections changed")
    if spec.get("blocked"):  # the model cannot be had: nothing may stand in for it
        u = now.get(spec["blocked"], {"props": {}, "flags": {}})
        if any(u["props"].get(k) for k in ("Sim.Library", "Sim.Name", "Sim.Params", "Sim.Device")) and u["flags"].get("exclude_from_sim") != "yes": bad.append(f"{spec['blocked']} was given a model that is not its maker's")
        return bad
    deck, err = refs.export(cli, d); L = lines(deck)
    if not L: return bad + [f"no SPICE netlist: {err[:200]}"]
    for name, tok in L.items():
        if len(tok) == 2 and tok[1].startswith("__"): bad.append(f"{name} has no model and is not excluded from simulation")
    for r in spec.get("exclude", []):
        if r in L or "X" + r in L: bad.append(f"{r} does not simulate but is in the netlist")
    for r in was:
        if r not in spec.get("exclude", []) and r not in L and "X" + r not in L: bad.append(f"{r} is not in the netlist")
    if not any(n[0] in "VI" for n in L): bad.append("no source in the netlist")
    gnd = lambda n: n.upper() in ("0", "GND")
    for r, m in spec.get("model", {}).items():
        tok = L.get("X" + r)
        if not tok: bad.append(f"{r} is not a subcircuit in the netlist"); continue
        ports = m["ports"]; nodes = tok[1:1 + len(ports)]; name = tok[1 + len(ports)] if len(tok) > 1 + len(ports) else ""
        if name.upper() != m["subckt"].upper(): bad.append(f"{r}: model {name!r}, the task's is {m['subckt']}"); continue
        for k, w in enumerate(ports):  # each port on the node its pin is on: "0" is ground, [ref, i] the node of that two-pin part's pin i
            want = None if w == "0" else (L.get(w[0].upper()) or [None] * 3)[1 + w[1]]
            if (w == "0" and not gnd(nodes[k])) or (w != "0" and nodes[k] != want): bad.append(f"{r}: model port {k + 1} is on {nodes[k]}, its pin is on {'ground' if w == '0' else want}")
    o = ngs.run(os.path.dirname(os.path.dirname(cli)), deck, ["op"] if spec.get("op", True) else [], [], "ltpsa", cwd=d, timeout=300)
    if not o["ok"] or o["problems"]: bad.append("ngspice: " + "; ".join(p[:140] for p in o["problems"][:3]) if o["problems"] else "ngspice did not load the netlist")
    return bad

def check(task, d, fixtures, cli, text):
    """task: its tasks.json entry"""
    c = task["check"]; fx = f"{fixtures}/{task['fixture']}"
    if c.get("blocked"):
        got = result(text) or {}; bad = [f"{k}: a number was reported ({got[k]:g}) though the part's model was not to be had" for k in c["answer"] if k in got]
        if not re.search(r"log[ -]?in|sign[ -]?in|sign[ -]?up|account|regist|licen[cs]e|credential|password", text or "", re.I): bad.append("the answer does not say that the model needs a sign-in")
        return bad + design(c, d, fx, cli)
    return answer(c["answer"], text) + design(c, d, fx, cli)

if __name__ == "__main__":  # checks.py TASK RUN_DIR FIXTURES CLI ANSWER_FILE
    t = next(t for t in json.load(open(HERE + "/tasks.json")) if t["id"] == sys.argv[1])
    print(json.dumps(check(t, sys.argv[2], sys.argv[3], sys.argv[4], open(sys.argv[5]).read()), indent=1))
