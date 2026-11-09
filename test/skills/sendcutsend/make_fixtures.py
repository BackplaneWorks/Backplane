#!/usr/bin/env python3
"""Writes the test fixtures in fixtures/: the two sample parts (bracket.dxf, mdf_panel.dxf) and, with --record, trimmed
recordings of SendCutSend's three sources from the helper's cache (the materials, hardware and finishes the tests use).
The recordings are test data only: the skill always reads the live files. Run from anywhere:
    python3 test/skills/sendcutsend/make_fixtures.py [--record]"""
import json, math, os, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / "deploy/skills/sendcutsend/scripts"))
import scs_sheet as S  # noqa: E402

FIX = HERE / "fixtures"
KEEP = ["ALU-040", "ALU-063", "ALU-125", "MDF-125", "MDF-250", "ACRYLICCL-118", "SST-060", "CRS-059"]

# the L bracket in 5052 0.063" (1.6 mm): radius and K-factor as recorded from the specs (0.035", 0.42)
T, R, K = 1.6, 0.035 * 25.4, 0.42
W, BASE, UP = 80.0, 40.0, 30.0  # width, outside flange lengths (mm)


def bracket(base=BASE, up=UP, hole_from_bend=12.0, hole_r=2.25):
    setback = R + T
    ba = S.bend_allowance(90, R, T, K)
    y_bend = base - setback + ba / 2  # bend line, from the base's edge
    L = S.flat_length([base, up], 90, R, T, K)
    holes = [(14, y_bend - ba / 2 - hole_from_bend, hole_r), (W - 14, y_bend - ba / 2 - hole_from_bend, hole_r),
             (W / 2, L - 10, 0.172 * 25.4 / 2)]  # the upright: F-M3-1 flush nut hole
    return dict(units="mm", outline=[(0, 0), (W, 0), (W, L), (0, L)], circles=holes, bends=[((0, y_bend), (W, y_bend))]), L


def mdf_panel():
    loops = [S.rounded_rect(40, 60, 160, 140, 6),                       # a display window
             S.slot((230, 50), 40, 8, 0), S.slot((230, 150), 40, 8, 90),  # two slots
             S.poly_loop([(260, 80), (285, 80), (285, 120), (260, 120)])]  # a square cutout
    circles = [(10, 10, 2.5), (290, 10, 2.5), (290, 190, 2.5), (10, 190, 2.5), (100, 30, 11)]
    return dict(units="mm", outline=S.rounded_rect(0, 0, 300, 200, 5), circles=circles, loops=loops)


def record():
    cache = Path(os.environ.get("SCS_CACHE") or os.path.expanduser("~/.cache/backplane/sendcutsend"))
    cat = json.loads((cache / "sendcutsend-catalog.json").read_text())
    spec = json.loads((cache / "sendcutsend-specs.json").read_text())
    have = {m["sku"] for m in cat["materials"]}
    keep = [k for k in KEEP if k in have]
    missing = sorted(set(KEEP) - have)
    if missing:
        print("not in the catalog now:", missing)
    cat["materials"] = [m for m in cat["materials"] if m["sku"] in keep]
    cat["hardware"] = [h for h in cat["hardware"] if h["sku"] in ("F-M3-1", "F-M3-2", "S-M3-0", "BSO-M3-6")] or cat["hardware"][:4]
    spec["materials"] = [m for m in spec["materials"] if m["sku"] in keep]
    for o in (cat, spec):
        o["_meta"]["recorded_for_tests"] = "trimmed copy; the skill reads the live file"
    (FIX / "sendcutsend-catalog.json").write_text(json.dumps(cat, indent=1) + "\n")
    (FIX / "sendcutsend-specs.json").write_text(json.dumps(spec, indent=1) + "\n")
    guide = (cache / "sendcutsend-ordering-guide.md").read_text().split("\n## Material Selection")[0]
    (FIX / "sendcutsend-ordering-guide.md").write_text(guide.rstrip() + "\n")


if __name__ == "__main__":
    FIX.mkdir(exist_ok=True)
    kw, L = bracket()
    S.write_dxf(FIX / "bracket.dxf", **kw)
    S.write_dxf(FIX / "mdf_panel.dxf", **mdf_panel())
    print(f"bracket flat length {L:.2f} mm")
    if "--record" in sys.argv:
        record()
