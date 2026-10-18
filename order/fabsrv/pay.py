"""Stripe Checkout.

A quote's Buy now opens a Checkout Session for the quote's lines; Stripe
collects the card, email, phone and the shipping address (only in the
quote's country, since shipping and duties were priced for it). Payment is
confirmed by asking Stripe for the session, never by trusting the return URL
alone, so no webhook endpoint is needed: the return page and a ten-minute
sweep both ask.
"""
import time

import httpx

from . import config, store

API = "https://api.stripe.com/v1"

TERMS = ("Prices are estimates from current manufacturing rates and are not guaranteed. If the real cost is higher, "
         "we'll ask before charging more, and if you'd rather not pay it you get a full refund. If it's lower, we refund "
         "the difference.")


class Unavailable(Exception):
    pass


def _client():
    if not config.STRIPE_KEY:
        raise Unavailable("payments are not set up yet")
    return httpx.Client(base_url=API, auth=(config.STRIPE_KEY, ""), timeout=20)


def _session(c, sid):
    r = c.get(f"/checkout/sessions/{sid}")
    r.raise_for_status()
    return r.json()


def checkout(row, quote):
    """The URL of an open Checkout for this quote: the one already open, or a new one."""
    qid = row["id"]
    with _client() as c:
        for o in store.q("select * from orders where quote_id = ? and status = 'checkout' order by created desc", qid):
            s = _session(c, o["session"])
            if s.get("status") == "open" and s.get("url"):
                return s["url"]
            _apply(o, s)
        if store.one("select 1 from orders where quote_id = ? and status not in ('checkout','cancelled')", qid):
            return f"{config.PUBLIC_URL}/q/{qid}/done?s=" + store.order_for(qid)["session"]
        data = {
            "mode": "payment",
            "client_reference_id": qid,
            "metadata[quote]": qid,
            "payment_intent_data[description]": f"Backplane order {qid}",
            "payment_intent_data[metadata][quote]": qid,
            "shipping_address_collection[allowed_countries][0]": row["country"],
            "phone_number_collection[enabled]": "true",
            "billing_address_collection": "auto",
            "custom_text[submit][message]": config.PILOT_NOTE if config.PILOT else TERMS,
            "success_url": f"{config.PUBLIC_URL}/q/{qid}/done?s={{CHECKOUT_SESSION_ID}}",
            "cancel_url": f"{config.PUBLIC_URL}/q/{qid}/cancelled",
            "expires_at": str(int(min(time.time() + 23 * 3600, max(row["expires"], time.time() + 3600)))),
        }
        lines = [ln for ln in quote["lines"] if round(ln["amount"] * 100) > 0]
        for i, ln in enumerate(lines):
            data[f"line_items[{i}][quantity]"] = "1"
            data[f"line_items[{i}][price_data][currency]"] = config.CURRENCY
            data[f"line_items[{i}][price_data][unit_amount]"] = str(round(ln["amount"] * 100))
            data[f"line_items[{i}][price_data][product_data][name]"] = ln["label"]
            if ln.get("detail"):
                data[f"line_items[{i}][price_data][product_data][description]"] = ln["detail"][:300]
        r = c.post("/checkout/sessions", data=data,
                   headers={"Idempotency-Key": f"{qid}-{int(time.time() // 60)}"})
        if r.status_code >= 400:
            print("stripe:", r.status_code, r.text[:500], flush=True)
            raise Unavailable("the payment page could not be opened; try again in a minute")
        s = r.json()
        store.create_order(qid, s["id"], s.get("amount_total") or 0)
        return s["url"]


def _apply(o, s):
    if s.get("payment_status") == "paid" and o["status"] == "checkout":
        det = s.get("customer_details") or {}
        ship = (s.get("collected_information") or {}).get("shipping_details") or s.get("shipping_details") or {}
        store.set_order(o["id"], status="paid", amount=s.get("amount_total") or o["amount"],
                        email=det.get("email") or "", shipping={"name": ship.get("name"), "address": ship.get("address"),
                                                               "phone": det.get("phone")})
    elif s.get("status") == "expired" and o["status"] == "checkout":
        store.set_order(o["id"], status="cancelled")


def confirm(o):
    try:
        with _client() as c:
            _apply(o, _session(c, o["session"]))
    except (Unavailable, httpx.HTTPError) as e:
        print("stripe confirm:", repr(e), flush=True)


def reconcile():
    if not config.STRIPE_KEY:
        return
    for o in store.open_checkouts():
        confirm(o)
