"""What can be ordered, in our own words.

Clients and agents only ever see these names; the fab pack (vendor_api.py)
maps them onto a fab's own option labels. Keeping the vocabulary ours means a
second fab is a second pack, not a change to every client.

A part is {kind, qty, options, features}:
- options: choices the person makes (colour, finish, material, ...)
- features: what the design measures (size, volume, cut length, ...); for an
  estimate the client sends them, for a quote the server measures the files
  and its numbers win.
"""

BOARD_COLORS = ["green", "black", "white", "blue", "red", "yellow", "purple"]
BOARD_FINISHES = ["hasl", "hasl_lead_free", "enig", "osp"]

PRINT_MATERIALS = {
    "sla": ["standard_resin", "black_resin", "tough_resin", "high_temp_resin", "clear_resin"],
    "mjf": ["pa12", "pa11", "pa12_glass"],
    "sls": ["pa12", "pa12_glass"],
    "fdm": ["abs", "asa", "pla", "tpu", "pa12_cf", "pc_abs", "peek"],
    "slm": ["stainless_316l", "titanium"],
}
# the colours each material comes in (the first is the default)
PRINT_COLORS = {
    ("sla", "standard_resin"): ["white"], ("sla", "black_resin"): ["black"], ("sla", "tough_resin"): ["green"],
    ("sla", "high_temp_resin"): ["yellow"], ("sla", "clear_resin"): ["translucent", "transparent"],
    ("mjf", "pa12"): ["black", "gray"], ("mjf", "pa11"): ["black", "gray"], ("mjf", "pa12_glass"): ["black", "white"],
    ("sls", "pa12"): ["black"], ("sls", "pa12_glass"): ["white"],
    ("fdm", "abs"): ["black", "white"], ("fdm", "asa"): ["black", "white"], ("fdm", "pla"): ["black", "white", "red", "blue"],
    ("fdm", "tpu"): ["black", "white", "gray", "translucent", "green"], ("fdm", "pa12_cf"): ["black"], ("fdm", "pc_abs"): ["black"],
    ("fdm", "peek"): ["brown"], ("slm", "stainless_316l"): ["metal"], ("slm", "titanium"): ["metal"],
}

CNC_MATERIALS = ["aluminum_6061", "aluminum_7075", "stainless_304", "steel_45", "steel_q235", "brass", "copper",
                 "abs_black", "abs_white", "pom_black", "pom_white", "pmma", "nylon", "polycarbonate", "fr4"]
CNC_FINISHES = ["none", "anodize", "bead_blast", "bead_blast_anodize", "hard_anodize", "conductive_anodize",
                "brushed", "brushed_anodize", "laser_mark"]
CNC_TOLERANCES = ["standard", "0.1", "0.05", "0.02"]

SHEET_MATERIALS = ["aluminum_5052", "stainless_304", "stainless_201", "steel_cold_rolled", "steel_galvanized"]
SHEET_FINISHES = ["none", "powder_coat", "anodize", "conductive_anodize", "hard_anodize"]


def _choice(key, choices, default):
    return {"key": key, "type": "choice", "choices": choices, "default": default}


def _num(key, lo, hi, default=None, unit=""):
    return {"key": key, "type": "number", "min": lo, "max": hi, "default": default, "unit": unit}


def _bool(key, default=False):
    return {"key": key, "type": "bool", "default": default}


KINDS = {
    "pcb": {
        "label": "Circuit board",
        "options": [
            _choice("layers", [1, 2, 4, 6, 8, 10, 12, 14, 16], 2),
            _choice("thickness_mm", [0.4, 0.6, 0.8, 1.0, 1.2, 1.6, 2.0], 1.6),
            _choice("color", BOARD_COLORS, "green"),
            _choice("finish", BOARD_FINISHES, "hasl"),
            _choice("copper_oz", [1, 2], 1),
            _bool("castellated"),
            _bool("gold_fingers"),
        ],
        "features": [_num("width_mm", 5, 1200, unit="mm"), _num("height_mm", 5, 1200, unit="mm"),
                     _num("min_drill_mm", 0.1, 6.5, 0.3, "mm")],
    },
    "3dp": {
        "label": "3D print",
        "options": [
            _choice("process", list(PRINT_MATERIALS), "mjf"),
            _choice("material", sorted({m for ms in PRINT_MATERIALS.values() for m in ms}), None),
            _choice("color", sorted({c for cs in PRINT_COLORS.values() for c in cs}), None),
        ],
        "features": [_num("volume_cm3", 0.001, 200000, unit="cm3"), _num("size_x_mm", 1, 2000, unit="mm"),
                     _num("size_y_mm", 1, 2000, unit="mm"), _num("size_z_mm", 1, 2000, unit="mm"),
                     _num("area_cm2", 0, 1000000, 0, "cm2")],
    },
    "cnc": {
        "label": "CNC part",
        "options": [
            _choice("material", CNC_MATERIALS, "aluminum_6061"),
            _choice("finish", CNC_FINISHES, "none"),
            _choice("tolerance", CNC_TOLERANCES, "standard"),
        ],
        "features": [_num("volume_cm3", 0.001, 200000, unit="cm3"), _num("size_x_mm", 1, 2000, unit="mm"),
                     _num("size_y_mm", 1, 2000, unit="mm"), _num("size_z_mm", 1, 2000, unit="mm"),
                     _num("area_cm2", 0, 1000000, unit="cm2")],
    },
    "sheetmetal": {
        "label": "Sheet metal part",
        "options": [
            _choice("material", SHEET_MATERIALS, "aluminum_5052"),
            _num("thickness_mm", 0.5, 6.0, 1.5, "mm"),
            _choice("finish", SHEET_FINISHES, "none"),
        ],
        "features": [_num("flat_x_mm", 1, 3000, unit="mm"), _num("flat_y_mm", 1, 3000, unit="mm"),
                     _num("cut_length_mm", 0, 1000000, 0, "mm"), _num("holes", 0, 100000, 0),
                     _num("bends", 0, 200, 0), _num("countersinks", 0, 10000, 0)],
    },
}

# the smallest order a fab takes, per kind (a pack can raise these)
MIN_QTY = {"pcb": 5, "3dp": 1, "cnc": 1, "sheetmetal": 1}
# fewest boards assembled in one order
MIN_ASSEMBLED = 2


def schema():
    return {k: {"label": v["label"], "options": v["options"], "features": v["features"], "min_qty": MIN_QTY[k]}
            for k, v in KINDS.items()}


def clean(kind, options, features, need_features=True):
    """Defaults filled in, values checked. Returns (options, features, problems);
    unknown keys are problems, never silently dropped."""
    k = KINDS.get(kind)
    if k is None:
        return {}, {}, [f"unknown kind: {kind!r} (one of {', '.join(KINDS)})"]
    problems = []
    out = []
    for given, fields, what in ((options or {}, k["options"], "option"), (features or {}, k["features"], "feature")):
        if not isinstance(given, dict):
            problems.append(f"{what}s must be an object")
            given = {}
        keys = {f["key"]: f for f in fields}
        for key in given:
            if key not in keys:
                problems.append(f"unknown {what} for {kind}: {key}")
        vals = {}
        for f in fields:
            v = given.get(f["key"], f.get("default"))
            if v is None:
                if what == "feature" and need_features:
                    problems.append(f"missing {f['key']} for {kind}")
                continue
            if f["type"] == "choice" and v not in f["choices"]:
                problems.append(f"{f['key']}={v!r} is not one of {f['choices']}")
                continue
            if f["type"] == "number":
                if isinstance(v, bool) or not isinstance(v, (int, float)):
                    problems.append(f"{f['key']} must be a number")
                    continue
                if not (f["min"] <= v <= f["max"]):
                    problems.append(f"{f['key']}={v} outside {f['min']}..{f['max']}")
                    continue
            if f["type"] == "bool" and not isinstance(v, bool):
                problems.append(f"{f['key']} must be true or false")
                continue
            vals[f["key"]] = v
        out.append(vals)
    opts, feats = out
    if kind == "3dp":
        proc = opts.get("process", "mjf")
        mats = PRINT_MATERIALS[proc]
        opts.setdefault("material", mats[0])
        if opts["material"] not in mats:
            problems.append(f"material {opts['material']!r} is not made with {proc} (one of {mats})")
            return opts, feats, problems
        cols = PRINT_COLORS[(proc, opts["material"])]
        # a colour the material does not come in becomes the material's own
        # (the quote's summary says which)
        if opts.get("color") not in cols:
            opts["color"] = cols[0]
    return opts, feats, problems
