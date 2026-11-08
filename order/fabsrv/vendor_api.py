"""The fab pack: how a fab prices things.

The pack is not part of this repository. It is a Python package named
`vendor`, installed on the server at $FAB_VENDOR (a directory holding
vendor/__init__.py), and it implements the functions below with the fab's
own price models and services. Everything it receives and returns is in the
words of catalog.py; it alone knows the fab's option labels.

    estimate(kind, options, features, qty) -> Price
        One part (pcb, 3dp, cnc, sheetmetal) at qty pieces, bare: no assembly,
        parts, shipping or tax.

    min_qty(kind, options) -> int
        The smallest order the fab takes for that part.

    assembly(spec) -> Price
        Assembly service fees for one board design, without the components.
        spec: boards, sides (1|2), smt_joints, tht_joints (per board),
        unique_parts, extended_parts, standard_only (the MPNs of stocked parts
        the fab places only with its standard service), xray_parts (how many
        of those a board places: the fab X-rays each), and the board's
        options and features (eligibility for the cheaper service depends on
        them all).

    library(mpns) -> {mpn: Stocked | None}
        The fab's own parts stock, by manufacturer part number (case-folded).
        Stocked: {code, mpn, maker, library: "basic"|"preferred"|"extended",
        stock, breaks: [[qty, unit_usd], ...], loss, least, standard_only}
        where a line of n placements buys max(n + loss, least) pieces, and
        standard_only is true for a part only the standard assembly service
        places (BGA, LGA, modules...).

    sourcing_fees(lines) -> Price
        What the fab charges to buy parts it does not stock (lines:
        [{mpn, qty, unit_usd, category}]), on top of the parts themselves.

    shipping(country, parcels) -> Ship
        parcels: [{kind, weight_kg, value_usd, assembled}] (assembled: how
        many boards of it come assembled, 0 for none); Ship: {price, method,
        days, ddp, parcels} for them all to a country, as the fab's checkout
        charges it (ddp: every parcel goes duty paid, so the quote carries
        the duties; else the courier collects them; parcels: how many it
        ships as), or {problem} when it cannot be priced now.

Price: {total, lines: [{label, amount}], lead_days, warnings, model_date}
"""
import importlib
import sys

from . import config

REQUIRED = ("estimate", "min_qty", "assembly", "library", "sourcing_fees", "shipping")
_pack = None


def pack():
    global _pack
    if _pack is None:
        if config.VENDOR and config.VENDOR not in sys.path:
            sys.path.insert(0, config.VENDOR)
        mod = importlib.import_module("vendor")
        missing = [f for f in REQUIRED if not hasattr(mod, f)]
        if missing:
            raise RuntimeError(f"fab pack is missing {', '.join(missing)}")
        _pack = mod
    return _pack


def available():
    try:
        pack()
        return True
    except Exception:
        return False
