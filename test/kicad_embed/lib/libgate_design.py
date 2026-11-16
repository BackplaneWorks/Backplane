"""The libraries gate's fresh design (bp.13 / Backplane 0.16.0), the same steps on Linux, macOS and Windows.

The person's bar: "DRC and ERC are clean", on a fresh small design that uses a standard part and a Backplane-library part:

  1. A new project: an empty schematic and an empty board with a 30 x 20 mm Edge.Cuts rectangle (`make_project`). The board's
     header (its layers and setup) is the seed board's own, with every item taken out, so each platform's KiCad reads its own
     format's layer table.
  2. The agent's kicad_edit on the schematic: C1 = Device:C on Capacitor_SMD:C_0603_1608Metric and C2 = Device:C on
     Backplane_Passives_Nominal:C_0805_2012Metric, each pin joined by `pins` to net N1 (pin 1) or N2 (pin 2): the two
     capacitors in parallel, two nets, passive pins only (`schematic_ops`).
  3. The board updated from the schematic: the schematic's netlist (kicad-cli sch export netlist) imported by the agent's
     kicad_edit `{ipc: ImportNetlist}`, matched by reference (`netlist_ops`).
  4. The agent's kicad_edit on the board: C1 and C2 moved inside the rectangle, then N1 and N2 routed (`board_ops`).
  5. The checks, every finding counting:
     - kicad-cli sch erc (all severities): 0 errors, 0 warnings;
     - kicad-cli pcb drc --schematic-parity (all severities): 0 violations, 0 unconnected, 0 parity;
     - in the editor: the agent's kicad_ipc RunChecks (FK_PCB with parity, then FK_SCH), then GetMarkers for each: no marker.
     (`count_cli`, `count_markers`.)

Each platform's harness drives it with its own `mcp(name, args) -> the tool's answer text` (the stand-in agent's MCP call) and
its own kicad-cli; the steps, part ids, nets, places and the pass bar are this file's, so the three platforms run the same thing.
"""
import json
import os
import re
import uuid

NAME = "libgate"
RECT = (100.0, 100.0, 130.0, 120.0)  # Edge.Cuts: x0, y0, x1, y1 (mm)
PLACES = {"C1": [108.0, 110.0], "C2": [122.0, 110.0]}  # inside RECT
PARTS = [
    {"ref": "C1", "lib": "Device:C", "value": "100n", "at": [101.6, 101.6], "footprint": "Capacitor_SMD:C_0603_1608Metric"},
    {"ref": "C2", "lib": "Device:C", "value": "1u", "at": [127.0, 101.6], "footprint": "Backplane_Passives_Nominal:C_0805_2012Metric"},
]
NETS = {"1": "N1", "2": "N2"}

_DROP = {"footprint", "segment", "via", "arc", "zone", "gr_line", "gr_rect", "gr_arc", "gr_circle", "gr_poly", "gr_curve", "gr_text",
         "gr_text_box", "dimension", "group", "generated", "image", "target", "table", "embedded_files", "net"}


def _forms(text):
    """The top-level forms inside the outer one: (start, end, head) each."""
    out, depth, i, n, start, instr = [], 0, 0, len(text), None, False
    while i < n:
        c = text[i]
        if instr:
            if c == "\\":
                i += 2
                continue
            if c == '"':
                instr = False
        elif c == '"':
            instr = True
        elif c == "(":
            depth += 1
            if depth == 2:
                start = i
        elif c == ")":
            if depth == 2 and start is not None:
                head = re.match(r"\(\s*([A-Za-z_0-9]+)", text[start:start + 64])
                out.append((start, i + 1, head.group(1) if head else ""))
                start = None
            depth -= 1
        i += 1
    return out


def empty_board(seed_pcb_text):
    """The seed board with every item taken out (its header, layers and setup kept), net 0 only, and RECT on Edge.Cuts."""
    t = seed_pcb_text
    keep, last = [], 0
    for s, e, head in _forms(t):
        drop = head in _DROP and not (head == "net" and re.match(r'\(\s*net\s+0\s', t[s:e]))
        if drop:
            keep.append(t[last:s].rstrip(" \t"))
            last = e
    keep.append(t[last:])
    body = "".join(keep)
    x0, y0, x1, y1 = RECT
    rect = (f'\t(gr_rect\n\t\t(start {x0} {y0})\n\t\t(end {x1} {y1})\n\t\t(stroke\n\t\t\t(width 0.05)\n\t\t\t(type default)\n\t\t)\n'
            f'\t\t(fill no)\n\t\t(layer "Edge.Cuts")\n\t\t(uuid "{uuid.uuid4()}")\n\t)\n')
    end = body.rstrip().rfind(")")
    return body[:end].rstrip() + "\n" + rect + ")\n"


def empty_schematic(seed_sch_text):
    """An empty schematic in the seed's own file version."""
    v = re.search(r"\(version\s+(\d+)\)", seed_sch_text)
    gv = re.search(r'\(generator_version\s+"([^"]+)"\)', seed_sch_text)
    return (f'(kicad_sch\n\t(version {v.group(1) if v else 20250114})\n\t(generator "eeschema")\n'
            + (f'\t(generator_version "{gv.group(1)}")\n' if gv else "")
            + f'\t(uuid "{uuid.uuid4()}")\n\t(paper "A4")\n\t(lib_symbols)\n\t(sheet_instances\n\t\t(path "/"\n\t\t\t(page "1")\n\t\t)\n\t)\n)\n')


def make_project(proj_dir, seed_pro, seed_pcb, seed_sch, name=NAME):
    """The new project's three files in proj_dir (made if missing); returns (pro, sch, pcb)."""
    os.makedirs(proj_dir, exist_ok=True)
    pro, sch, pcb = (os.path.join(proj_dir, f"{name}.{x}") for x in ("kicad_pro", "kicad_sch", "kicad_pcb"))
    try:
        p = json.load(open(seed_pro, encoding="utf-8"))
    except (OSError, ValueError):
        p = {}
    p.setdefault("meta", {})["filename"] = f"{name}.kicad_pro"
    p.pop("sheets", None)
    json.dump(p, open(pro, "w", encoding="utf-8"), indent=2)
    with open(pcb, "w", encoding="utf-8", newline="\n") as f:
        f.write(empty_board(open(seed_pcb, encoding="utf-8").read()))
    with open(sch, "w", encoding="utf-8", newline="\n") as f:
        f.write(empty_schematic(open(seed_sch, encoding="utf-8").read()))
    return pro, sch, pcb


def schematic_ops():
    return [{"add": [{"symbol": {**{k: p[k] for k in ("lib", "ref", "value", "at", "footprint")}, "pins": NETS}} for p in PARTS]}]


def netlist_ops(netlist_path):
    return [{"ipc": {"type": "kiapi.board.commands.ImportNetlist", "message": {
        "board": "$doc", "netlist_path": netlist_path, "match_mode": "NMM_REFERENCE", "update_footprints": True, "delete_extra_footprints": True}}}]


def board_ops():
    return [{"move": ref, "to": at} for ref, at in PLACES.items()] + [{"route": sorted(set(NETS.values())), "width": 0.25}]


def count_cli(report):
    """Every finding in a kicad-cli ERC or DRC JSON report: {violations, unconnected, parity, types}."""
    v = list(report.get("violations", []) or [])
    for sh in report.get("sheets", []) or []:
        v += sh.get("violations", []) or []
    u = report.get("unconnected_items", []) or []
    p = report.get("schematic_parity", []) or []
    return {"violations": len(v), "unconnected": len(u), "parity": len(p),
            "types": sorted({x.get("type", "") for x in v + u + p})}


def count_markers(answer):
    """The markers in a GetMarkers answer (the MCP tool's text: JSON, maybe nested): {count, types}."""
    def find(o):
        if isinstance(o, dict):
            if "markers" in o:
                return o["markers"]
            for x in o.values():
                r = find(x)
                if r is not None:
                    return r
        if isinstance(o, list):
            for x in o:
                r = find(x)
                if r is not None:
                    return r
        if isinstance(o, str) and "markers" in o:
            try:
                return find(json.loads(o))
            except ValueError:
                return None
        return None
    m = find(answer)
    if m is None:
        return None
    return {"count": len(m), "types": sorted({x.get("type", "") for x in m})}
