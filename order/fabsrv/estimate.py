"""The "from $X" number: each part bare (no assembly, parts, shipping or
tax) at the smallest order the fab takes, with our fee. Built from the
features a client sends; no files are involved.
"""
import functools
import json

from . import catalog, config, vendor_api


class Unavailable(Exception):
    pass


def fee_rate(kind):
    return config.FEE_CNC if kind == "cnc" else config.FEE


@functools.lru_cache(maxsize=4096)
def _one(kind, opts_json, feats_json):
    v = vendor_api.pack()
    opts, feats = json.loads(opts_json), json.loads(feats_json)
    qty = max(catalog.MIN_QTY[kind], int(v.min_qty(kind, opts)))
    p = v.estimate(kind, opts, feats, qty)
    return qty, float(p["total"]), int(p.get("lead_days") or 0), tuple(p.get("warnings") or ())


def estimate(parts):
    try:
        vendor_api.pack()
    except Exception:
        raise Unavailable()
    out, total = [], 0.0
    for p in parts[:catalog_max()]:
        if not isinstance(p, dict):
            continue
        name = str(p.get("name", ""))[:64]
        kind = p.get("kind")
        opts, feats, problems = catalog.clean(kind, p.get("options"), p.get("features"))
        if problems:
            out.append({"name": name, "kind": kind, "problems": problems})
            continue
        qty, price, days, warnings = _one(kind, json.dumps(opts, sort_keys=True), json.dumps(feats, sort_keys=True))
        price = round(price * (1 + fee_rate(kind)), 2)
        total += price
        out.append({"name": name, "kind": kind, "qty": qty, "from": price, "lead_days": days,
                    "warnings": list(warnings)})
    priced = [p for p in out if "from" in p]
    return {"currency": config.CURRENCY, "from": round(total, 2) if priced else None, "parts": out,
            "label": label(total) if priced else "",
            "note": "Bare parts at the smallest order, before shipping and tax."}


def label(x):
    return f"from ${x:,.0f}" if x >= 100 else f"from ${x:,.2f}"


def catalog_max():
    return 20
