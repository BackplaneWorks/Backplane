"""What can be ordered, in our own words.

Clients and agents only ever see these names; the fab pack (vendor_api.py)
maps them onto a fab's own option labels. Keeping the vocabulary ours means a
second fab is a second pack, not a change to every client.

A part is {kind, qty, options, features}:
- options: choices the person makes (colour, finish, material, ...)
- features: what the design measures (size, volume, cut length, ...); for an
  estimate the client sends them, for a quote the server measures the files
  and its numbers win.

Boards come in several kinds (BOARDS): rigid FR-4 ("pcb"), flex, HDI,
aluminium and copper core, and the high-frequency Rogers and PTFE. They
share the board's features and files; only rigid FR-4 boards are assembled.
"""

BOARD_COLORS = ["green", "black", "white", "blue", "red", "yellow", "purple"]
BOARD_FINISHES = ["hasl", "hasl_lead_free", "enig", "osp"]

# the kinds that are boards (made from gerbers), and the one that is assembled
BOARDS = ("pcb", "flex", "hdi", "aluminum", "copper_core", "rogers", "ptfe")
ASSEMBLED = ("pcb",)

# flex: the total thickness each layer count comes in (it sets the substrate
# and the copper; the first is the default)
FLEX_THICKNESS = {1: [0.07, 0.11, 0.12, 0.14], 2: [0.11, 0.12, 0.19, 0.2, 0.24], 4: [0.2, 0.25, 0.3, 0.35, 0.45]}
FLEX_STIFFENERS = ["none", "polyimide_0.1", "polyimide_0.15", "polyimide_0.2", "polyimide_0.225", "polyimide_0.25",
                   "fr4_0.1", "fr4_0.2", "fr4_0.4", "fr4_0.6", "fr4_0.8", "fr4_1.0", "fr4_1.2", "fr4_1.6",
                   "steel_0.1", "steel_0.2", "steel_0.3", "tape_3m468", "tape_tesa8854", "tape_3m9077"]
# HDI: the thicknesses each layer count comes in (1.6 mm by default), and
# its build-up steps (1 at 4 layers, up to 2 at 6, up to 3 from 8)
HDI_THICKNESS = {4: [0.8, 1.0, 1.2, 1.6, 2.0], 6: [0.6, 0.8, 1.0, 1.2, 1.6, 1.8, 2.0, 2.5, 3.0],
                 8: [0.8, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.5, 3.0], 10: [0.8, 1.0, 1.2, 1.6, 1.8, 2.0, 2.5, 3.0],
                 12: [1.0, 1.2, 1.6, 2.0, 2.5, 3.0], 14: [1.2, 1.6, 2.0], 16: [1.6, 1.8, 2.0, 2.5, 3.0]}


def hdi_steps(layers):
    return [1] if layers <= 4 else ([1, 2] if layers == 6 else [1, 2, 3])

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


# what every board kind measures: its Edge.Cuts box and smallest plated hole
BOARD_FEATURES = [_num("width_mm", 5, 1200, unit="mm"), _num("height_mm", 5, 1200, unit="mm"),
                  _num("min_drill_mm", 0.1, 6.5, 0.3, "mm")]

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
        "features": BOARD_FEATURES,
    },
    "flex": {
        "label": "Flex board",
        "options": [
            _choice("layers", [1, 2, 4], 2),
            _choice("thickness_mm", sorted({t for ts in FLEX_THICKNESS.values() for t in ts}), None),
            _choice("coverlay", ["yellow", "black", "white"], "yellow"),
            _choice("stiffener", FLEX_STIFFENERS, "none"),
            _choice("emi_shield", ["none", "one_side", "both_sides"], "none"),
            _choice("gold_uin", [1, 2], 1),
            _bool("gold_fingers"),
        ],
        "features": BOARD_FEATURES,
    },
    "hdi": {
        "label": "HDI board",
        "options": [
            _choice("layers", [4, 6, 8, 10, 12, 14, 16], 4),
            _choice("thickness_mm", sorted({t for ts in HDI_THICKNESS.values() for t in ts}), 1.6),
            _choice("steps", [1, 2, 3], 1),
            _choice("laminate", ["np175f", "s1000_2m"], "np175f"),
            _choice("color", BOARD_COLORS, "green"),
            _choice("finish", BOARD_FINISHES, "hasl"),
            _choice("via_fill", ["plugged", "untented", "epoxy_filled", "copper_filled"], "plugged"),
            _bool("castellated"),
            _bool("gold_fingers"),
            _bool("edge_plating"),
        ],
        "features": BOARD_FEATURES,
    },
    "aluminum": {
        "label": "Aluminium board",
        "options": [
            _choice("layers", [1], 1),
            _choice("thickness_mm", [1.6, 0.8, 1.0, 1.2], 1.6),
            _choice("color", ["white", "black"], "white"),
            _choice("finish", ["hasl", "hasl_lead_free"], "hasl"),
        ],
        "features": BOARD_FEATURES,
    },
    "copper_core": {
        "label": "Copper-core board",
        "options": [
            _choice("layers", [1], 1),
            _choice("thickness_mm", [1.6, 1.0, 1.2], 1.6),
            _choice("color", ["white", "black"], "white"),
            _choice("finish", ["osp"], "osp"),
        ],
        "features": BOARD_FEATURES,
    },
    "rogers": {
        "label": "Rogers board",
        "options": [
            _choice("layers", [2], 2),
            _choice("thickness_mm", [0.51, 0.76, 1.52], 0.51),
            _choice("color", ["green", "purple"], "green"),
            _choice("finish", ["enig", "osp"], "enig"),
            _choice("gold_uin", [1, 2], 1),
            _choice("via_fill", ["tented", "untented"], "tented"),
        ],
        "features": BOARD_FEATURES,
    },
    "ptfe": {
        "label": "PTFE board",
        "options": [
            _choice("layers", [2], 2),
            _choice("thickness_mm", [0.76, 1.52], 0.76),
            _choice("laminate", ["dk2.94", "dk3.0", "dk2.65", "dk2.55"], "dk2.94"),
            _choice("color", ["green"], "green"),
            _choice("finish", ["enig", "osp"], "enig"),
            _choice("gold_uin", [1, 2], 1),
            _choice("via_fill", ["tented", "untented"], "tented"),
        ],
        "features": BOARD_FEATURES,
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
MIN_QTY = {**{k: 5 for k in BOARDS}, "3dp": 1, "cnc": 1, "sheetmetal": 1}
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
    if kind in ("flex", "hdi"):
        problems += board_rules(kind, opts, options if isinstance(options, dict) else {})[0]
    # 0.8 mm aluminium only comes lead-free (the quote's summary says so)
    if kind == "aluminum" and opts.get("thickness_mm") == 0.8 and opts.get("finish") == "hasl":
        opts["finish"] = "hasl_lead_free"
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


def _a(layers):
    return "an" if str(layers).startswith("8") or layers in (11, 18) else "a"


def _words(vals):
    return " or ".join(str(v).replace("_", " ") for v in vals)


def hdi_only(layers):
    """What a many-layer HDI board is made with, where fewer layers have a choice."""
    only = {}
    if layers >= 6:
        only["finish"] = ["enig", "osp"]
        only["via_fill"] = ["epoxy_filled"]
    if layers >= 8:
        only["laminate"] = ["s1000_2m"]
    return only


def board_rules(kind, opts, given=()):
    """What a flex or HDI board's layer count allows: its thickness and HDI's
    steps must fit (problems otherwise); a value it can't have that the
    person didn't choose (not in given) becomes the first it can. Returns
    (problems, [(key, was, now)]). Run again once the gerbers have said how many
    layers there are."""
    layers = opts.get("layers")
    problems, changed = [], []
    if kind == "flex":
        ts = FLEX_THICKNESS.get(layers)
        if ts is None:
            return problems, changed
        opts.setdefault("thickness_mm", ts[0])
        if opts["thickness_mm"] not in ts and "thickness_mm" not in given:
            changed.append(("thickness_mm", opts["thickness_mm"], ts[0]))
            opts["thickness_mm"] = ts[0]
        if opts["thickness_mm"] not in ts:
            problems.append(f"{_a(layers)} {layers}-layer flex board is {', '.join(f'{t:g}' for t in ts)} mm thick, not {opts['thickness_mm']:g}")
        return problems, changed
    ts = HDI_THICKNESS.get(layers)
    if ts is None:
        return problems, changed
    if opts.get("thickness_mm") not in ts:
        problems.append(f"{_a(layers)} {layers}-layer HDI board is {', '.join(f'{t:g}' for t in ts)} mm thick, not {opts.get('thickness_mm')}")
    steps = hdi_steps(layers)
    if opts.get("steps") not in steps:
        problems.append(f"{_a(layers)} {layers}-layer HDI board has {_words(steps)} build-up "
                        f"step{'' if steps == [1] else 's'}, not {opts.get('steps')}")
    for key, only in hdi_only(layers).items():
        if opts.get(key) in only:
            continue
        if key in given:
            problems.append(f"{_a(layers)} {layers}-layer HDI board's {key.replace('_', ' ')} is {_words(only)}, "
                            f"not {_words([opts.get(key)])}")
        else:
            changed.append((key, opts.get(key), only[0]))
            opts[key] = only[0]
    return problems, changed
