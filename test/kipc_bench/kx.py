"""What KiCad itself says about a project folder, for the design tasks' checks (checks.py) and their fixtures (mkfix2.py):
DRC and ERC as JSON, the schematic's netlist, every pad's place and net (IPC-D-356), and top-level blocks of a board file."""
import json, os, re, shutil, subprocess, tempfile, xml.etree.ElementTree as ET

def run(cli, *a):
    r = subprocess.run([cli, *a], capture_output=True, text=True)
    return r
def drc(cli, pcb, refill=False):
    with tempfile.TemporaryDirectory() as t:
        o = t + "/drc.json"; run(cli, "pcb", "drc", "--format", "json", *(["--refill-zones"] if refill else []), "-o", o, pcb)
        return json.load(open(o))
def erc(cli, sch):
    with tempfile.TemporaryDirectory() as t:
        o = t + "/erc.json"; run(cli, "sch", "erc", "--format", "json", "-o", o, sch)
        j = json.load(open(o)); return [v for s in j["sheets"] for v in s["violations"]]
def netlist(cli, sch):
    """{ref: {"lib", "value", "footprint", "pins": {pin: net}}} and {net: [(ref, pin)]}"""
    with tempfile.TemporaryDirectory() as t:
        o = t + "/n.xml"; run(cli, "sch", "export", "netlist", "--format", "kicadxml", "-o", o, sch); r = ET.parse(o).getroot()
    comps = {}
    for c in r.find("components"):
        ls = c.find("libsource")
        comps[c.get("ref")] = {"lib": (ls.get("lib") + ":" + ls.get("part")) if ls is not None else "", "value": c.findtext("value") or "", "footprint": c.findtext("footprint") or "", "pins": {}}
    nets = {}
    for n in r.find("nets"):
        nets[n.get("name")] = [(x.get("ref"), x.get("pin")) for x in n.findall("node")]
        for ref, pin in nets[n.get("name")]:
            if ref in comps: comps[ref]["pins"][pin] = n.get("name")
    return comps, nets
def pads(cli, pcb):
    """{(ref, pad): (x, y)} in mm, from KiCad's IPC-D-356 export (its net names are cut to 14 characters, so not used)"""
    with tempfile.TemporaryDirectory() as t:
        o = t + "/b.d356"; run(cli, "pcb", "export", "ipcd356", "-o", o, pcb); txt = open(o).read()
    out = {}
    for l in txt.splitlines():
        if not l.startswith("317") and not l.startswith("327"): continue
        ref, pad = l[20:26].strip(), l[27:31].strip().lstrip("-")
        m = re.search(r"X([+-]\d+)Y([+-]\d+)", l)
        if m and ref: out[(ref, pad)] = (int(m.group(1)) * 0.00254, -int(m.group(2)) * 0.00254)
    return out
def nettag(d):
    m = re.search(r"\[([^\]]+)\]", d); return m.group(1) if m else ""
def blocks(text):
    """the top-level children of a board or sheet file: [(start, end, head)] where head is e.g. 'segment'"""
    out = []; depth = 0; s = None; i = 0; n = len(text); instr = False
    while i < n:
        c = text[i]
        if instr:
            if c == "\\": i += 2; continue
            if c == '"': instr = False
        elif c == '"': instr = True
        elif c == "(":
            depth += 1
            if depth == 2: s = i
        elif c == ")":
            if depth == 2:
                head = re.match(r"\(\s*([^\s()]+)", text[s:s + 64]).group(1); out.append((s, i + 1, head))
            depth -= 1
        i += 1
    return out
