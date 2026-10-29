"""From measured files to a quote.

The files decide: a board's layers and size come from its gerbers, a solid's
volume and box from its STEP, a flat pattern from its DXF. What the client
said is only used where the files cannot tell (finish, colour, material,
bends) and a difference is reported as a warning.

The person sees three lines and a total: manufacturing, shipping, and duties
and taxes. Our fee (FEE of everything, FEE_CNC of CNC parts) is part of the
manufacturing line, not a line of its own; shipping and taxes show at cost.
"""
import re
import datetime

from . import catalog, config, duties, sourcing, vendor_api

# g/cm3, for the parcel's weight
DENSITY = {"aluminum": 2.7, "stainless": 7.95, "steel": 7.85, "brass": 8.5, "copper": 8.96, "titanium": 4.43,
           "abs": 1.05, "asa": 1.07, "pla": 1.24, "tpu": 1.2, "pa12": 1.01, "pa11": 1.03, "pc": 1.2, "peek": 1.3,
           "pom": 1.41, "pmma": 1.18, "nylon": 1.14, "polycarbonate": 1.2, "fr4": 1.85, "resin": 1.15, "metal": 7.95}
BOARD_DENSITY = 1.85
# a board's own, where it isn't FR-4's (polyimide, aluminium and copper cores, PTFE)
BOARD_DENSITIES = {"flex": 1.42, "aluminum": 2.7, "copper_core": 8.9, "ptfe": 2.2}



# the person never sees who makes it: notes from the fab's own price models
# name it, so its names become "the fab" before anything is shown
_FAB = re.compile(r"\b(?:JLCPCB|JLC3DP|JLCCNC|JLCMC|JLC|LCSC|EasyEDA)(?:'s)?\b", re.I)


def made_of(parts):
    """What the Manufacturing line covers, by what is ordered: "Boards, assembly and components
    and 3D prints", "3D prints", ..."""
    kinds = [p.get("kind") for p in parts]
    words = []
    if any(k in catalog.BOARDS for k in kinds):
        words.append("boards, assembly and components" if any(p.get("assembled") for p in parts) else "boards")
    for k, w in (("3dp", "3D prints"), ("cnc", "CNC parts"), ("sheetmetal", "sheet metal")):
        if k in kinds:
            words.append(w)
    if not words:
        return "Parts"
    text = words[0] if len(words) == 1 else ", ".join(words[:-1]) + " and " + words[-1]
    return text[0].upper() + text[1:]

def neutral(text):
    return _FAB.sub(lambda m: "the fab's" if m.group(0).lower().endswith("'s") else "the fab", str(text))

def density(material):
    m = (material or "").lower()
    for k, v in DENSITY.items():
        if m.startswith(k) or k in m:
            return v
    return 1.2


FINISH = {"hasl": "HASL", "hasl_lead_free": "lead-free HASL", "enig": "ENIG", "osp": "OSP"}
# what each board kind is called in a quote's summary (rigid FR-4 is just a board)
A_BOARD = {"pcb": "a board", "flex": "a flex board", "hdi": "an HDI board", "aluminum": "an aluminium board",
           "copper_core": "a copper-core board", "rogers": "a Rogers board", "ptfe": "a PTFE board"}
BOARD_WORD = {"pcb": "board", "flex": "flex board", "hdi": "HDI board", "aluminum": "aluminium board",
              "copper_core": "copper-core board", "rogers": "Rogers RO4350B board", "ptfe": "PTFE board"}


def board_summary(kind, opts, feats):
    size = f"{feats.get('width_mm', 0):g}×{feats.get('height_mm', 0):g} mm"
    thick = f"{opts['thickness_mm']:g} mm thick"
    if kind == "flex":
        stiff = "" if opts["stiffener"] == "none" else ", " + stiffener(opts["stiffener"]) + " stiffener"
        emi = {"none": "", "one_side": ", EMI shield on one side", "both_sides": ", EMI shield on both sides"}[opts["emi_shield"]]
        return f"{opts['layers']}-layer flex board, {size}, {thick}, {opts['coverlay']} coverlay{stiff}{emi}"
    if kind == "hdi":
        return (f"{opts['layers']}-layer HDI board ({opts['steps']}-step), {size}, {thick}, {opts['color']}, "
                f"{FINISH[opts['finish']]}")
    if kind == "ptfe":
        return f"PTFE board (Dk {opts['laminate'][2:]}), {size}, {thick}, {opts['color']}, {FINISH[opts['finish']]}"
    if kind in ("aluminum", "copper_core", "rogers"):
        return f"{BOARD_WORD[kind][0].upper()}{BOARD_WORD[kind][1:]}, {size}, {thick}, {opts['color']}, {FINISH[opts['finish']]}"
    return f"{opts['layers']}-layer board, {size}, {thick}, {opts['color']}, {FINISH[opts['finish']]}"


def stiffener(v):
    """A flex stiffener in words: "fr4_0.2" is "0.2 mm FR-4", "tape_3m468" is "3M 468 tape"."""
    what, _, mm = v.partition("_")
    if what == "tape":
        return {"3m468": "3M 468", "tesa8854": "tesa 8854", "3m9077": "3M 9077"}.get(mm, mm) + " tape"
    return f"{mm} mm " + {"polyimide": "polyimide", "fr4": "FR-4", "steel": "stainless steel"}.get(what, what)


def summary(kind, opts, feats):
    if kind in catalog.BOARDS:
        return board_summary(kind, opts, feats)
    if kind == "3dp":
        return f"3D print, {opts['process'].upper()} {pretty(opts['material'])}, {opts['color']}"
    if kind == "cnc":
        fin = "" if opts["finish"] == "none" else ", " + pretty(opts["finish"])
        return f"CNC, {pretty(opts['material'])}{fin}"
    fin = "" if opts["finish"] == "none" else ", " + pretty(opts["finish"])
    return f"Sheet metal, {pretty(opts['material'])} {opts['thickness_mm']:g} mm{fin}"


NAMES = {"pa12": "PA12 nylon", "pa11": "PA11 nylon", "pa12_glass": "glass-filled PA12", "standard_resin": "standard resin",
         "black_resin": "black resin", "tough_resin": "tough resin", "high_temp_resin": "high-temperature resin",
         "clear_resin": "clear resin", "abs": "ABS", "asa": "ASA", "pla": "PLA", "tpu": "TPU", "pa12_cf": "carbon-filled PA12",
         "pc_abs": "PC-ABS", "peek": "PEEK", "stainless_316l": "316L stainless", "titanium": "titanium",
         "aluminum_6061": "6061 aluminium", "aluminum_7075": "7075 aluminium", "stainless_304": "304 stainless",
         "stainless_201": "201 stainless", "steel_45": "45 steel", "steel_q235": "Q235 steel", "brass": "brass", "copper": "copper",
         "abs_black": "black ABS", "abs_white": "white ABS", "pom_black": "black POM", "pom_white": "white POM", "pmma": "acrylic",
         "nylon": "nylon", "polycarbonate": "polycarbonate", "fr4": "FR4", "aluminum_5052": "5052 aluminium",
         "steel_cold_rolled": "cold-rolled steel", "steel_galvanized": "galvanized steel", "powder_coat": "powder coat",
         "anodize": "anodized", "bead_blast": "bead blasted", "bead_blast_anodize": "bead blasted and anodized",
         "hard_anodize": "hard anodized", "conductive_anodize": "conductive anodized", "brushed": "brushed",
         "brushed_anodize": "brushed and anodized", "laser_mark": "laser marked",
         "epoxy_filled": "epoxy-filled and capped", "s1000_2m": "S1000-2M laminate"}


def pretty(v):
    return NAMES.get(v, str(v).replace("_", " "))


def _board(v, p, f, problems, warnings):
    kind = p["kind"]
    opts = dict(p["options"])
    layers = f.get("layers")
    if layers and layers != opts["layers"]:
        allowed = catalog.KINDS[kind]["options"][0]["choices"]
        if layers in allowed:
            warnings.append(f"{p['name']}: the gerbers have {layers} copper layers (you said {opts['layers']}); priced as {layers}")
            opts["layers"] = layers
            if kind in ("flex", "hdi"):
                probs, changed = catalog.board_rules(kind, opts, p.get("given", ()))
                problems += [f"{p['name']}: {x}" for x in probs]
                if changed:
                    warnings.append(f"{p['name']}: at {layers} layers it comes with " + ", ".join(
                        (f"{now:g} mm thickness" if key == "thickness_mm" else f"{key.replace('_', ' ')} {FINISH.get(now) or pretty(now)}")
                        for key, _, now in changed))
        elif kind == "pcb":
            problems.append(f"{p['name']}: {layers} copper layers can't be made")
        else:
            has = " or ".join(str(x) for x in allowed) if len(allowed) <= 2 else f"{allowed[0]} to {allowed[-1]}"
            problems.append(f"{p['name']}: the gerbers have {layers} copper layers, and {A_BOARD[kind]} has {has}")
    feats = {"width_mm": f.get("width_mm"), "height_mm": f.get("height_mm"), "min_drill_mm": f.get("min_drill_mm") or 0.3}
    if not feats["width_mm"]:
        return None
    if feats["width_mm"] < 5 or feats["height_mm"] < 5 or max(feats["width_mm"], feats["height_mm"]) > 600:
        problems.append(f"{p['name']}: a {feats['width_mm']}×{feats['height_mm']} mm board is outside what can be made")
        return None
    if f.get("min_drill_mm") is not None and f["min_drill_mm"] < 0.15:
        problems.append(f"{p['name']}: a {f['min_drill_mm']} mm drill is smaller than the 0.15 mm minimum")
    qty = max(p["qty"], v.min_qty(kind, opts), catalog.MIN_QTY[kind])
    if qty != p["qty"]:
        warnings.append(f"{p['name']}: boards come in at least {qty}; priced for {qty}")
    pr = v.estimate(kind, opts, feats, qty)
    warnings += [f"{p['name']}: {w}" for w in pr.get("warnings", [])]
    return opts, feats, qty, pr


def price(manifest, measured):
    v = vendor_api.pack()
    country = manifest["ship_to"]
    problems, warnings = list(measured.get("problems", [])), list(measured.get("warnings", []))
    mfg = cnc = 0.0
    lead = 0
    parcels, parts_out, part_lines = [], [], []
    for p in manifest["parts"]:
        f = measured.get("parts", {}).get(p["name"], {})
        kind = p["kind"]
        if kind in catalog.BOARDS:
            b = _board(v, p, f, problems, warnings)
            if b is None:
                continue
            opts, feats, qty, pr = b
            cost = float(pr["total"])
            lead = max(lead, int(pr.get("lead_days") or 0))
            out = {"name": p["name"], "kind": kind, "qty": qty, "summary": summary(kind, opts, feats)}
            if p.get("assemble"):
                n = p["assemble"]
                comp = sourcing.components(v, f.get("bom", []), n)
                problems += [f"{p['name']}: {x}" for x in comp["problems"]]
                warnings += [f"{p['name']}: {x}" for x in comp["warnings"]]
                sides = 2 if f.get("paste_bottom") or f.get("placed_bottom") else 1
                asm = v.assembly({"boards": n, "sides": sides, "smt_joints": (f.get("paste_top", 0) + f.get("paste_bottom", 0)),
                                  "tht_joints": f.get("tht_holes", 0), "unique_parts": comp["unique"],
                                  "extended_parts": comp["extended"], "options": opts, "features": feats})
                warnings += [f"{p['name']}: {w}" for w in asm.get("warnings", [])]
                cost += float(asm["total"]) + comp["total"]
                lead = max(lead, int(pr.get("lead_days") or 0) + int(asm.get("lead_days") or 0) + comp["extra_days"])
                out["assembled"] = n
                out["sourcing"] = comp["lines"]
                part_lines.append({"label": f"{p['name']}: assembly of {n} and parts", "amount": round(float(asm["total"]) + comp["total"], 2)})
            part_lines.append({"label": f"{p['name']}: {qty} boards", "amount": round(float(pr["total"]), 2)})
            grams = (feats["width_mm"] * feats["height_mm"] / 100 * opts["thickness_mm"] / 10
                     * BOARD_DENSITIES.get(kind, BOARD_DENSITY) * qty)
            parcels.append({"kind": kind, "weight_kg": grams / 1000 * (1.3 if p.get("assemble") else 1.0), "value_usd": cost})
            mfg += cost
            parts_out.append(out)
            continue
        opts = dict(p["options"])
        feats = dict(p.get("features") or {})
        for k, val in f.items():
            if k in ("volume_cm3", "area_cm2", "size_x_mm", "size_y_mm", "size_z_mm", "flat_x_mm", "flat_y_mm",
                     "cut_length_mm", "holes"):
                feats[k] = val
        need = [x["key"] for x in catalog.KINDS[kind]["features"] if x.get("default") is None]
        missing = [k for k in need if not feats.get(k)]
        if missing:
            problems.append(f"{p['name']}: could not measure {', '.join(missing)}")
            continue
        qty = max(p["qty"], v.min_qty(kind, opts))
        pr = v.estimate(kind, opts, feats, qty)
        warnings += [f"{p['name']}: {w}" for w in pr.get("warnings", [])]
        cost = float(pr["total"])
        lead = max(lead, int(-(-float(pr.get("lead_days") or 0) // 1)))
        if kind == "cnc":
            cnc += cost
        else:
            mfg += cost
        vol = feats.get("volume_cm3") or (feats.get("flat_x_mm", 0) * feats.get("flat_y_mm", 0) / 100 * opts.get("thickness_mm", 1) / 10)
        parcels.append({"kind": kind, "weight_kg": vol * density(opts.get("material")) * qty / 1000, "value_usd": cost})
        part_lines.append({"label": f"{p['name']}: {qty} pieces", "amount": round(cost, 2)})
        parts_out.append({"name": p["name"], "kind": kind, "qty": qty, "summary": summary(kind, opts, feats)})

    goods = mfg + cnc
    ship = v.shipping(country, parcels) if parcels else {"price": 0.0, "method": "", "days": "", "taxes": None, "ddp": True}
    if ship.get("problem"):
        problems.append(ship["problem"])
    ddp = bool(ship.get("ddp"))
    tax = duties.estimate(country, goods, ship.get("price", 0.0), len(parts_out), ddp)
    # the fee is folded into the manufacturing line (the same amount as when
    # it was a line of its own, so the total does not change)
    fee = config.FEE * (mfg + ship["price"] + tax) + config.FEE_CNC * cnc
    lines = [
        {"label": "Manufacturing", "amount": round(goods + fee, 2), "detail": made_of(parts_out)},
        {"label": "Shipping", "amount": round(ship["price"], 2), "detail": neutral(", ".join(x for x in (ship.get("method"), ship.get("days")) if x))},
        {"label": "Duties & taxes", "amount": round(tax, 2), "detail": duties.note(country, ddp)},
    ]
    total = round(sum(ln["amount"] for ln in lines), 2)
    q = {"title": manifest.get("title") or "", "country": country, "country_name": duties.name(country),
         "currency": config.CURRENCY, "parts": parts_out, "lines": lines, "total": total,
         "part_lines": part_lines, "lead_days": lead, "problems": [neutral(x) for x in problems], "warnings": [neutral(x) for x in warnings],
         "shipping": {"method": neutral(ship.get("method", "")), "days": ship.get("days", ""), "ddp": ddp},
         "priced_at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")}
    q["markdown"] = markdown(q)
    q["pay_label"] = f"Buy now · {money(total)}"
    return q


def money(x):
    return f"${x:,.2f}"


def markdown(q):
    """The quote as the buy thread shows it (the hub adds the Pay link)."""
    out = [f"**Your quote: {money(q['total'])}**", ""]
    ship = q["shipping"]
    how = f" by {ship['method']}" if ship.get("method") else ""
    out.append(f"Ships to {q['country_name']}{how}" + (f", {ship['days']}" if ship.get("days") else "")
               + (f". Made in about {q['lead_days']} days." if q["lead_days"] else "."))
    out += ["", "| Item | Cost |", "|---|---:|"]
    for ln in q["lines"]:
        out.append(f"| {ln['label']} | {money(ln['amount'])} |")
    out.append(f"| **Total** | **{money(q['total'])}** |")
    parts = []
    for p in q["parts"]:
        parts.append(f"{p['name']} ×{p['qty']}" + (f" ({p['assembled']} assembled)" if p.get("assembled") else ""))
    if parts:
        out += ["", "Parts: " + " · ".join(parts)]
    if q["problems"]:
        out += ["", "**To fix before ordering**"] + [f"- {x}" for x in q["problems"]]
        return "\n".join(out)
    if q["warnings"]:
        out += ["", "Notes:"] + [f"- {w}" for w in q["warnings"][:6]]
    if config.PILOT:
        out += ["", "**Pilot:** ordering through Backplane is still being tested. Buy now opens a test checkout and your card "
                    "will not be charged. To get the parts made now, order them from any fabricator with the files exported to `mfg/`."]
        return "\n".join(out)
    out += ["", f"Valid for {config.QUOTE_DAYS} days. Prices are estimates and not guaranteed: if the real cost turns out "
                "higher we'll ask before charging more, and you can have a full refund instead; if it's lower, we refund the difference."]
    return "\n".join(out)
