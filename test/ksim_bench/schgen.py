"""A KiCad schematic written by a script, for the simulation benchmark's fixtures (mkfix.py): real library symbols from the runtime, every pin
joined to its net by a short wire and a label (a power symbol for a supply net), the way many boards' sheets are drawn. Nothing is rotated, so a
pin's place is the symbol's place plus the pin's own.
  s = Sch(runtime, "name"); s.part("Device:R", "R1", "10k", 100, 80, {"1": "IN", "2": "OUT"}, fp="Resistor_SMD:R_0603_1608Metric"); s.write(dir)
The file is written in the runtime's current format and then upgraded by its own kicad-cli (as test/kipc_bench/mkfix.sh does), so a later save
changes only what an edit changed."""
import json, math, os, re, subprocess, sys, uuid
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, os.path.join(HERE, "..", "kipc_bench"))
import kx, norm  # the IPC benchmark's s-expression reader and what KiCad's CLI says

POWER = {"GND": "power:GND", "+5V": "power:+5V", "+3V3": "power:+3V3", "+12V": "power:+12V"}
FONT = "(effects (font (size 1.27 1.27)))"

def q(s): return '"' + str(s).replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'

class Sch:
    def __init__(self, rt, name):
        self.rt, self.name, self.libs, self.items, self.n, self.pwr = rt, name, {}, [], 0, 0
        self.root = self.uid("root"); self.want = {}  # (ref, pin) -> net, what the netlist must say afterwards
    def uid(self, what=""):
        self.n += 1; return str(uuid.uuid5(uuid.NAMESPACE_URL, f"backplane-ksim-bench/{self.name}/{self.n}/{what}"))
    def _block(self, lib, sym):
        t = open(f"{self.rt}/share/kicad/symbols/{lib}.kicad_symdir/{sym}.kicad_sym").read()
        return next(t[s:e] for s, e, h in kx.blocks(t) if h == "symbol")
    def lib(self, lib_id):
        """the library symbol as the schematic keeps it (one that extends another is written out whole), and its pins [(number, x, y, angle)]"""
        if lib_id in self.libs: return self.libs[lib_id][1]
        lib, sym = lib_id.split(":"); b = self._block(lib, sym); m = re.search(r'\(extends "([^"]+)"\)', b)
        if m:
            par = m.group(1); p = self._block(lib, par); kids = kx.blocks(p); props = [(s, e) for s, e, h in kids if h == "property"]
            mine = "\n\t\t".join(b[s:e] for s, e, h in kx.blocks(b) if h == "property")
            p = p[:props[0][0]] + mine + p[props[-1][1]:]  # the parent's texts (one run of property blocks) give way to this symbol's own
            b = p.replace(f'(symbol "{par}"', f'(symbol "{sym}"', 1).replace(f'(symbol "{par}_', f'(symbol "{sym}_')
        tree = norm.parse(b); pins = []
        for u in norm.kids(tree, "symbol"):
            for p in norm.kids(u, "pin"):
                a = norm.kid(p, "at"); pins.append((norm.val(p, "number"), float(a[1]), float(a[2]), int(float(a[3]))))
        self.libs[lib_id] = (b.replace(f'(symbol "{sym}"', f"(symbol {q(lib_id)}", 1), pins); return pins
    def prop(self, k, v, x, y, hide=False, just=""):
        return f"(property {q(k)} {q(v)} (at {x:g} {y:g} 0) {'(hide yes) ' if hide else ''}(show_name no) (do_not_autoplace no) (effects (font (size 1.27 1.27)){just}))"
    def _sym(self, lib_id, ref, value, x, y, fields, flags):
        pins = self.lib(lib_id)
        ps = [self.prop("Reference", ref, x + 3.81, y - 1.27, ref.startswith("#"), " (justify left)"), self.prop("Value", value, x + 3.81, y + 1.27, False, " (justify left)")]
        for k in ("Footprint", "Datasheet", "Description"): ps.append(self.prop(k, fields.get(k, ""), x, y, True))
        ps += [self.prop(k, v, x, y, True) for k, v in fields.items() if k not in ("Footprint", "Datasheet", "Description")]
        pn = " ".join(f"(pin {q(p[0])} (uuid {q(self.uid())}))" for p in pins)
        self.items.append(f"(symbol (lib_id {q(lib_id)}) (at {x:g} {y:g} 0) (unit 1) (body_style 1) {flags} (uuid {q(self.uid(ref))})\n\t\t" + "\n\t\t".join(ps) + f"\n\t\t{pn}\n\t\t"
                          f"(instances (project {q(self.name)} (path {q('/' + self.root)} (reference {q(ref)}) (unit 1)))))")
        return pins
    def part(self, lib_id, ref, value, x, y, nets=None, fp="", fields=None, sim=None, exclude=False, board=True):
        """a part at (x, y) mm; nets: {pin number: net name}; sim: its Sim.* fields; exclude: left out of simulation; board=False: a part that is
        only there for simulation (out of the BOM, off the board)"""
        f = {"Footprint": fp, **(fields or {}), **(sim or {})}; yn = lambda b: "yes" if b else "no"
        pins = self._sym(lib_id, ref, value, x, y, f, f"(exclude_from_sim {yn(exclude)}) (in_bom {yn(board)}) (on_board {yn(board)}) (in_pos_files {yn(board)}) (dnp no)")
        for num, px, py, ang in pins:
            net = (nets or {}).get(num); ax, ay = x + px, y - py
            if net is None: self.items.append(f"(no_connect (at {ax:g} {ay:g}) (uuid {q(self.uid())}))"); continue
            if not ref.startswith("#"): self.want[(ref, num)] = net
            ex, ey = ax - 2.54 * round(math.cos(math.radians(ang))), ay + 2.54 * round(math.sin(math.radians(ang)))
            self.items.append(f"(wire (pts (xy {ax:g} {ay:g}) (xy {ex:g} {ey:g})) (stroke (width 0) (type default)) (uuid {q(self.uid())}))")
            if net in POWER:
                self.pwr += 1; self._sym(POWER[net], f"#PWR{self.pwr:02d}", net, ex, ey, {}, "(exclude_from_sim no) (in_bom yes) (on_board yes) (in_pos_files yes) (dnp no)")
            else: self.items.append(f"(label {q(net)} (at {ex:g} {ey:g} 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid {q(self.uid())}))")
    def text(self, s, x, y):
        self.items.append(f"(text {q(s)} (exclude_from_sim no) (at {x:g} {y:g} 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid {q(self.uid())}))")
    def write(self, d, cli):
        os.makedirs(d, exist_ok=True); sch = f"{d}/{self.name}.kicad_sch"
        open(sch, "w").write(f'(kicad_sch (version 20261025) (generator "eeschema") (generator_version "10.99") (uuid {q(self.root)}) (paper "A4")\n\t(lib_symbols\n\t\t'
            + "\n\t\t".join(b for b, _ in self.libs.values()) + "\n\t)\n\t" + "\n\t".join(self.items) + '\n\t(sheet_instances (path "/" (page "1")))\n\t(embedded_fonts no)\n)\n')
        json.dump({"meta": {"filename": self.name + ".kicad_pro", "version": 3}, "schematic": {"legacy_lib_dir": "", "legacy_lib_list": []}, "sheets": [[self.root, "Root"]]}, open(f"{d}/{self.name}.kicad_pro", "w"), indent=2)
        r = kx.run(cli, "sch", "upgrade", "--force", sch)
        if r.returncode: raise SystemExit(f"{sch}: {r.stdout}{r.stderr}")
        comps, _ = kx.netlist(cli, sch)  # what was drawn is what was meant: every pin on its net, and no two nets joined by a stray wire end
        for (ref, pin), net in self.want.items():
            got = comps.get(ref, {}).get("pins", {}).get(pin, "")
            if got.lstrip("/") != net: raise SystemExit(f"{sch}: {ref}.{pin} is on {got!r}, meant {net!r}")
        return sch
