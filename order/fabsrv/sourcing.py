"""Components for an assembled board.

Each BOM line is looked up in the fab's own stock first (the pack's library()), by
its stock code when the BOM has one, else by manufacturer part number. A line the
fab stocks is bought as the fab buys it: max(placements + attrition, minimum). A
line it does not stock is bought for the order from a distributor (Mouser here),
at the distributor's price for that quantity plus a margin for the fab's sourcing
(SOURCED_MARKUP), with spare parts, and its fees (the pack's sourcing_fees()).
A line found nowhere is a problem: the board must change before it can be made.

Mouser allows 30 calls a minute and 1000 a day; ten part numbers go in one call,
answers are kept for hours, and the day's budget is guarded.
"""
import json
import math
import re
import threading
import time
import urllib.request
import xml.etree.ElementTree as ET

from . import config

SOURCED_MARKUP = float(config.env("FAB_SOURCED_MARKUP", "1.25"))
SOURCED_DAYS = 15
_lock = threading.Lock()
_cache = {}
_calls = []
TTL = 12 * 3600


def price_at(breaks, qty):
    """The unit price for qty from [(min qty, price), ...] (the last break at or below qty)."""
    unit = None
    for q, p in sorted(breaks):
        if qty >= q or unit is None:
            unit = p
    return unit or 0.0


# ------------------------------------------------------------------ money

_fx = {"t": 0.0, "eur": 1.08}


def eur_usd():
    """ECB's daily reference rate, kept for a day (1.08 if it cannot be had)."""
    if time.time() - _fx["t"] < 86400:
        return _fx["eur"]
    try:
        with urllib.request.urlopen("https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml", timeout=10) as r:
            root = ET.fromstring(r.read())
        for el in root.iter():
            if el.get("currency") == "USD":
                _fx["eur"] = float(el.get("rate"))
                break
    except Exception:
        pass
    _fx["t"] = time.time()
    return _fx["eur"]


def money(text, currency):
    """'1.234,56 €' or '$1,234.56' as a float in USD."""
    s = re.sub(r"[^\d.,]", "", text or "")
    if not s:
        return None
    if "," in s and "." in s:
        s = s.replace(".", "").replace(",", ".") if s.rfind(",") > s.rfind(".") else s.replace(",", "")
    elif "," in s:
        s = s.replace(",", ".")
    try:
        v = float(s)
    except ValueError:
        return None
    return v * eur_usd() if (currency or "").upper() == "EUR" else v


# ------------------------------------------------------------------ Mouser

def _budget():
    now = time.time()
    with _lock:
        while _calls and _calls[0] < now - 86400:
            _calls.pop(0)
        if len(_calls) >= 950:
            return False
        recent = [t for t in _calls if t > now - 60]
        if len(recent) >= 28:
            time.sleep(max(0.0, 61 - (now - recent[0])))
        _calls.append(time.time())
        return True


def _mouser_call(mpns):
    if not config.MOUSER_KEY or not _budget():
        return None
    body = json.dumps({"SearchByPartRequest": {"mouserPartNumber": "|".join(mpns), "partSearchOptions": "Exact"}}).encode()
    req = urllib.request.Request(f"https://api.mouser.com/api/v1/search/partnumber?apiKey={config.MOUSER_KEY}", body,
                                 {"Content-Type": "application/json", "Accept": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            data = json.load(r)
    except Exception:
        return None
    return ((data.get("SearchResults") or {}).get("Parts")) or []


def _offer(p):
    breaks = []
    for b in p.get("PriceBreaks") or []:
        v = money(b.get("Price"), b.get("Currency"))
        if v is not None:
            breaks.append((int(b.get("Quantity") or 1), v))
    try:
        stock = int(p.get("AvailabilityInStock") or 0)
    except ValueError:
        stock = 0
    return {"mpn": p.get("ManufacturerPartNumber") or "", "maker": p.get("Manufacturer") or "", "stock": stock,
            "breaks": breaks, "min": int(p.get("Min") or 1), "mult": int(p.get("Mult") or 1),
            "category": p.get("Category") or ""}


def mouser(mpns):
    """{mpn: offer or None} for each part number: an exact match with prices, in stock first."""
    out, need = {}, []
    now = time.time()
    for m in dict.fromkeys(mpns):
        hit = _cache.get(("mouser", m.lower()))
        if hit and now - hit[0] < TTL:
            out[m] = hit[1]
        else:
            need.append(m)
    for i in range(0, len(need), 10):
        chunk = need[i:i + 10]
        parts = _mouser_call(chunk)
        if parts is None:
            for m in chunk:
                out[m] = None
            continue
        offers = [_offer(p) for p in parts]
        for m in chunk:
            same = [o for o in offers if o["mpn"].lower() == m.lower() and o["breaks"]]
            same.sort(key=lambda o: -o["stock"])
            best = same[0] if same else None
            _cache[("mouser", m.lower())] = (now, best)
            out[m] = best
    return out


# ------------------------------------------------------------------ the BOM

def components(v, lines, boards):
    keys = [(ln["code"] or ln["mpn"]) + ("" if ln["code"] else "\t" + (ln.get("maker") or "")) for ln in lines]
    lib = v.library(keys) if lines else {}
    total, extended, extra_days = 0.0, 0, 0
    rows, problems, warnings, sourced = [], [], [], []
    elsewhere = []
    for key, ln in zip(keys, lines):
        usage = ln["qty"] * boards
        st = lib.get(key)
        if st:
            buy = max(usage + st["loss"], st["least"], usage)
            if st["stock"] >= buy:
                total += price_at(st["breaks"], buy) * buy
                extended += 1 if st["library"] == "extended" else 0
                rows.append({"refs": ln["refs"][:8], "mpn": ln["mpn"] or st["mpn"], "status": "stocked", "library": st["library"]})
                continue
            warnings.append(f"{ln['mpn'] or st['mpn']}: only {st['stock']} in the fab's stock; sourced instead")
        elsewhere.append((ln, usage))
    offers = mouser([ln["mpn"] for ln, _ in elsewhere if ln["mpn"]]) if elsewhere else {}
    for ln, usage in elsewhere:
        o = offers.get(ln["mpn"]) if ln["mpn"] else None
        refs = ", ".join(ln["refs"][:4]) + ("…" if len(ln["refs"]) > 4 else "")
        if not o:
            problems.append(f"{refs} ({ln['mpn'] or ln['code']}) can't be bought from the fab's stock or a distributor; "
                            "choose a part that is in stock")
            rows.append({"refs": ln["refs"][:8], "mpn": ln["mpn"], "status": "missing"})
            continue
        buy = usage + max(2, math.ceil(usage * 0.03))
        buy = max(buy, o["min"])
        if o["mult"] > 1:
            buy = int(math.ceil(buy / o["mult"]) * o["mult"])
        if o["stock"] < buy:
            warnings.append(f"{ln['mpn']}: the distributor has {o['stock']}; lead time may be longer")
        unit = price_at(o["breaks"], buy) * SOURCED_MARKUP
        total += unit * buy
        extended += 1
        extra_days = SOURCED_DAYS
        sourced.append({"mpn": ln["mpn"], "qty": buy, "unit_usd": unit, "category": o["category"]})
        rows.append({"refs": ln["refs"][:8], "mpn": ln["mpn"], "status": "sourced"})
    if sourced:
        fees = v.sourcing_fees(sourced)
        total += float(fees["total"])
        warnings.append(f"{len(sourced)} part{'s' if len(sourced) != 1 else ''} bought from a distributor for this order "
                        f"(about {SOURCED_DAYS} more days)")
    return {"total": round(total, 2), "unique": len(lines), "extended": extended, "lines": rows,
            "problems": problems, "warnings": warnings, "extra_days": extra_days}
