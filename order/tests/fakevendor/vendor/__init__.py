"""A stand-in fab pack for the tests: simple, fixed prices in the server's vocabulary."""


def _p(total, days=3):
    return {"total": total, "lines": [{"label": "part", "amount": total}], "lead_days": days, "warnings": [], "model_date": "test"}


BOARDS = ("pcb", "flex", "hdi", "aluminum", "copper_core", "rogers", "ptfe")


def estimate(kind, options, features, qty):
    if kind in BOARDS:
        return _p(round(2 + features["width_mm"] * features["height_mm"] / 1000 * qty, 2))
    return _p(round(1 + features.get("volume_cm3", 1) * 0.5 * qty, 2))


def min_qty(kind, options):
    return 5 if kind in BOARDS else 1


def assembly(spec):
    return _p(10 + 0.01 * spec["smt_joints"] * spec["boards"], 2)


STOCK = {"RC0402-10K": {"code": "C1", "mpn": "RC0402-10K", "maker": "x", "library": "basic", "stock": 100000,
                        "breaks": [[1, 0.01]], "loss": 10, "least": 20}}


def library(keys):
    return {k: STOCK.get(k.partition("\t")[0]) for k in keys}


def sourcing_fees(lines):
    return {"total": 0.0, "lines": [], "lead_days": 0, "warnings": []}


def shipping(country, parcels):
    return {"price": 20.0, "method": "Test Express (duties paid)", "days": "3 days", "taxes": None, "ddp": True}
