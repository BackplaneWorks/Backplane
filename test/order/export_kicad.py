#!/usr/bin/env python3
"""What a buy thread's agent exports (Order.prompt.buy step 4), for test/order/buy_e2e.py's stand-in agent: one KiCad board's gerbers and drill
files from the saved board, with kicad-cli, into mfg/<stamp>/<name>/gerbers/, and the manifest listing every file by its sha256.

    export_kicad.py BOARD.kicad_pcb MFG_DIR --name "Main board" --qty 5 --ship-to US [--ship-region GA] [--layers N]

--fab-json FILE writes the board's fab file (kind pcb, measured from the exported outline) instead, as the design thread would have."""
import argparse, hashlib, json, os, pathlib, re, subprocess, sys

LAYERS = ["F.Cu", "B.Cu", "F.Mask", "B.Mask", "F.Paste", "B.Paste", "F.Silkscreen", "B.Silkscreen", "Edge.Cuts"]


def copper(pcb):
    """The board's copper layer count, from its (layers ...) list."""
    head = pathlib.Path(pcb).read_text(errors="replace")[:20000]
    return len(set(re.findall(r'\(\d+\s+"((?:F|B|In\d+)\.Cu)"', head))) or 2


def export(pcb, out, inner):
    g = out / "gerbers"; g.mkdir(parents=True, exist_ok=True)
    layers = LAYERS + [f"In{i}.Cu" for i in range(1, inner + 1)]
    subprocess.run(["kicad-cli", "pcb", "export", "gerbers", "--layers", ",".join(layers), "-o", str(g) + "/", pcb], check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["kicad-cli", "pcb", "export", "drill", "--format", "excellon", "--drill-origin", "absolute", "--excellon-units", "mm", "--excellon-separate-th",
                    "-o", str(g) + "/", pcb], check=True, stdout=subprocess.DEVNULL)
    for j in g.glob("*.gbrjob"): j.unlink()
    return g


def outline_mm(g):
    """The outline's bounding box (mm) from the Edge.Cuts gerber (FSLAX46, mm)."""
    f = next(p for p in g.iterdir() if "Edge_Cuts" in p.name)
    xs, ys = [], []
    for x, y in re.findall(r"X(-?\d+)Y(-?\d+)", f.read_text()): xs.append(int(x) / 1e6); ys.append(int(y) / 1e6)
    return round(max(xs) - min(xs), 2), round(max(ys) - min(ys), 2)


def main():
    a = argparse.ArgumentParser(); a.add_argument("pcb"); a.add_argument("out"); a.add_argument("--name", default="Main board"); a.add_argument("--qty", type=int, default=5)
    a.add_argument("--ship-to", default="US"); a.add_argument("--ship-region", default=""); a.add_argument("--title", default=""); a.add_argument("--fab-json")
    o = a.parse_args()
    n = copper(o.pcb)
    if o.fab_json:  # the design thread's fab file, measured from a throwaway export
        tmp = pathlib.Path(o.out); g = export(o.pcb, tmp, n - 2); w, h = outline_mm(g)
        json.dump({"name": o.name, "kind": "pcb", "source": os.path.basename(o.pcb), "qty": o.qty, "options": {"layers": n, "thickness_mm": 1.6},
                   "features": {"width_mm": w, "height_mm": h, "min_drill_mm": 0.3}}, open(o.fab_json, "w"), indent=2)
        return
    d = pathlib.Path(o.out); slug = re.sub(r"[^a-z0-9]+", "-", o.name.lower()).strip("-") or "board"
    g = export(o.pcb, d / slug, n - 2); w, h = outline_mm(g)
    files = {str(p.relative_to(d)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(g.iterdir()) if p.is_file()}
    man = {"spec": 1, "title": o.title or pathlib.Path(o.pcb).stem, "ship_to": o.ship_to, "parts": [{"name": o.name, "kind": "pcb", "qty": o.qty, "options": {"layers": n, "thickness_mm": 1.6},
           "features": {"width_mm": w, "height_mm": h, "min_drill_mm": 0.3}, "files": {"gerbers": f"{slug}/gerbers"}, "assemble": 0}], "files": files}
    if o.ship_region: man["ship_region"] = o.ship_region
    (d / "manifest.json").write_text(json.dumps(man, indent=2))
    print(f"exported {len(files)} files to {d}", file=sys.stderr)


if __name__ == "__main__":
    main()
