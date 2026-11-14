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
_calls = []  # (time, install) of each call to the distributor in the last 24 h
TTL = 12 * 3600
DAY_BUDGET = 950           # of the distributor's 1,000 a day
SHARE = DAY_BUDGET // 4    # most of the day's calls one quote, or one install, may spend
MAX_MPNS = 100             # unique part numbers a quote may have looked up at a distributor
UNAVAILABLE = object()     # an answer for a part number: the distributor could not be asked (not "not found")


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

def _budget(owner="", spent=None):
    """True if a call may be made: the day's budget, this install's share and this quote's share (spent,
    a one-item list counting the quote's calls) all have room."""
    now = time.time()
    with _lock:
        while _calls and _calls[0][0] < now - 86400:
            _calls.pop(0)
        if len(_calls) >= DAY_BUDGET:
            return False
        if owner and sum(1 for _, o in _calls if o == owner) >= SHARE:
            return False
        if spent is not None and spent[0] >= SHARE:
            return False
        recent = [t for t, _ in _calls if t > now - 60]
        if len(recent) >= 28:
            time.sleep(max(0.0, 61 - (now - recent[0])))
        _calls.append((time.time(), owner))
        if spent is not None:
            spent[0] += 1
        return True


def _mouser_call(mpns, owner="", spent=None):
    if not config.MOUSER_KEY or not _budget(owner, spent):
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


def mouser(mpns, owner=""):
    """{mpn: offer, None or UNAVAILABLE} for each part number: an exact match with prices, in stock first;
    None when the distributor answered and does not have it; UNAVAILABLE when it could not be asked (budget
    spent, a cap reached, no key, an error), which says nothing about the part."""
    out, need = {}, []
    now = time.time()
    spent = [0]
    for m in dict.fromkeys(mpns):
        hit = _cache.get(("mouser", m.lower()))
        if hit and now - hit[0] < TTL:
            out[m] = hit[1]
        else:
            need.append(m)
    for m in need[MAX_MPNS:]:
        out[m] = UNAVAILABLE
    need = need[:MAX_MPNS]
    for i in range(0, len(need), 10):
        chunk = need[i:i + 10]
        parts = _mouser_call(chunk, owner, spent)
        if parts is None:
            for m in chunk:
                out[m] = UNAVAILABLE
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

def components(v, lines, boards, install=""):
    keys = [(ln["code"] or ln["mpn"]) + ("" if ln["code"] else "\t" + (ln.get("maker") or "")) for ln in lines]
    lib = v.library(keys) if lines else {}
    total, extended, extra_days = 0.0, 0, 0
    rows, problems, warnings, sourced, standard_only, unavailable = [], [], [], [], [], []
    xray = 0  # placements a board of parts only standard assembly places (BGA, LGA...): the fab X-rays each
    elsewhere = []
    for key, ln in zip(keys, lines):
        usage = ln["qty"] * boards
        st = lib.get(key)
        if st:
            buy = max(usage + st["loss"], st["least"], usage)
            if st["stock"] >= buy:
                total += price_at(st["breaks"], buy) * buy
                extended += 1 if st["library"] == "extended" else 0
                if st.get("standard_only"):
                    standard_only.append(ln["mpn"] or st["mpn"])
                    xray += ln["qty"]
                rows.append({"refs": ln["refs"][:8], "mpn": ln["mpn"] or st["mpn"], "status": "stocked", "library": st["library"]})
                continue
            warnings.append(f"{ln['mpn'] or st['mpn']}: only {st['stock']} in the fab's stock; sourced instead")
        elsewhere.append((ln, usage))
    offers = mouser([ln["mpn"] for ln, _ in elsewhere if ln["mpn"]], install) if elsewhere else {}
    for ln, usage in elsewhere:
        o = offers.get(ln["mpn"]) if ln["mpn"] else None
        refs = ", ".join(ln["refs"][:4]) + ("…" if len(ln["refs"]) > 4 else "")
        if o is UNAVAILABLE:
            unavailable.append(f"{refs} ({ln['mpn']}): sourcing is unavailable right now, so this part can't be priced yet; "
                               "ask for the quote again later (nothing is wrong with the board)")
            rows.append({"refs": ln["refs"][:8], "mpn": ln["mpn"], "status": "unavailable"})
            continue
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
            "problems": problems, "unavailable": unavailable, "warnings": warnings, "extra_days": extra_days, "standard_only": standard_only, "xray_parts": xray}
