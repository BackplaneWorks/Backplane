"""The few HTML pages: one line on coming back from Stripe (paid or not),
errors, and the order list. A quote is only shown in Backplane; its Buy now
goes straight to Stripe. Everything shown is escaped; no scripts.
"""
import datetime
import html
import json

from . import config


CSS = """
:root{--bg:#f7f7f5;--card:#fff;--text:#1c1c1a;--muted:#6b6b66;--line:#e4e4df;--accent:#2f6f4f;--warn:#9a5b00;--bad:#a33}
@media (prefers-color-scheme:dark){:root{--bg:#141413;--card:#1e1e1c;--text:#ecece8;--muted:#9a9a94;--line:#33332f;--accent:#5fbf8f;--warn:#e0a040;--bad:#f07070}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:15px/1.5 system-ui,-apple-system,Segoe UI,sans-serif}
main{max-width:640px;margin:0 auto;padding:32px 16px 64px}
h1{font-size:22px;margin:0 0 4px}h2{font-size:15px;margin:24px 0 8px;color:var(--muted);font-weight:600}
.sub{color:var(--muted);margin:0 0 20px}
.card{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:16px 18px;margin:12px 0}
table{width:100%;border-collapse:collapse}td{padding:6px 0;vertical-align:top}td.n{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
tr.total td{border-top:1px solid var(--line);padding-top:10px;font-weight:700;font-size:17px}
.muted{color:var(--muted)}.small{font-size:13px}
button{background:var(--accent);color:#fff;border:0;border-radius:10px;padding:12px 20px;font-size:16px;font-weight:600;cursor:pointer;width:100%}
.bad{color:var(--bad)}.ok{color:var(--accent)}ul{margin:6px 0;padding-left:20px}
.admin td{border-bottom:1px solid var(--line);padding:8px 6px;font-size:13px}.admin input,.admin select{font:inherit}
"""


def e(s):
    return html.escape(str(s if s is not None else ""), quote=True)


def money(x):
    return f"${x:,.2f}"


def page(title, body):
    return (f"<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>"
            f"<meta name=robots content=noindex><title>{e(title)}</title><style>{CSS}</style></head><body><main>{body}</main></body></html>")


def order_status(s):
    return {"paid": "Paid. We're placing your order.", "placed": "Ordered. In production.",
            "shipped": "Shipped.", "delivered": "Delivered.", "refunded": "Refunded.",
            "cancelled": "Cancelled."}.get(s, s)


def done(status):
    if status == "checkout":
        return page("Payment", "<h1>Almost there</h1><p class=sub>Stripe hasn't confirmed the payment yet. "
                               "Reload in a moment; Backplane will show it too.</p>")
    if config.PILOT:
        return page("Payment", "<h1>Thank you</h1><p class=sub>This was a test: ordering through Backplane is a pilot, so your card "
                               "was not charged and nothing is ordered. To get your parts made, order them from any fabricator with "
                               "the files Backplane exported to the mfg folder of your project. You can close this tab.</p>")
    return page("Payment", f"<h1>Thank you</h1><p class=sub>{e(order_status(status))} "
                           "You can close this tab: Backplane shows the progress in your buy thread, and the receipt is in your email.</p>")


def cancelled():
    return page("Payment", "<h1>Not paid</h1><p class=sub>Nothing was charged. You can close this tab; Buy now in Backplane opens it again.</p>")


def error(code, msg):
    return page("Error", f"<h1>{e(code)}</h1><p class=sub>{e(msg)}</p>")


def _opts(p):
    o = p.get("options") or {}
    return ", ".join(f"{k.replace('_', ' ')} {v}" for k, v in o.items() if v not in (None, False, ""))


def admin(rows):
    """The paid orders, each with what to order where: its parts, options and costs, the
    customer's address, and the files to download."""
    out = ["<style>main{max-width:1100px}</style><h1>Orders</h1><p class=sub>Paid orders, newest first. "
           "Place each with the fab using its files and options, then set its status: the customer's buy thread follows it.</p>"]
    if not rows:
        out.append("<div class=card>No paid orders yet.</div>")
    for o in rows:
        q = json.loads(o["result"] or "{}")
        man = json.loads(o["manifest"] or "{}") if "manifest" in o.keys() else {}
        ship = json.loads(o["shipping"] or "{}")
        addr = ship.get("address") or {}
        a = ", ".join(str(addr.get(k)) for k in ("line1", "line2", "city", "state", "postal_code", "country") if addr.get(k))
        when = datetime.datetime.fromtimestamp(o["created"], datetime.timezone.utc).strftime("%-d %b %Y %H:%M UTC")
        parts = {p["name"]: p for p in man.get("parts", [])}
        rows_html = []
        for p in q.get("parts", []):
            mp = parts.get(p["name"], {})
            rows_html.append(f"<tr><td><b>{e(p['name'])}</b> × {e(p['qty'])}" + (f" ({e(p['assembled'])} assembled)" if p.get("assembled") else "")
                             + f"<br><span class='muted small'>{e(p.get('summary'))}<br>{e(_opts(mp))}</span></td></tr>")
        costs = "".join(f"<tr><td class=small>{e(ln['label'])}</td><td class='n small'>{money(ln['amount'])}</td></tr>" for ln in q.get("part_lines", []))
        costs += "".join(f"<tr><td class=small>{e(ln['label'])}</td><td class='n small'>{money(ln['amount'])}</td></tr>" for ln in q.get("lines", [])[1:3])
        opts = "".join(f"<option{' selected' if s == o['status'] else ''}>{s}</option>"
                       for s in ("paid", "placed", "shipped", "delivered", "refunded", "cancelled"))
        out.append(
            f"<div class=card><table><tr><td><b>{e(q.get('title') or o['quote_id'])}</b> · {money(o['amount'] / 100)} paid · {e(when)}"
            f"<br><span class='muted small'>{e(o['id'])} · {e(o['quote_id'])}</span></td>"
            f"<td class=n><a href='/admin/files/{e(o['quote_id'])}'>Files (.zip)</a></td></tr></table>"
            f"<table style='margin-top:10px'><tr><td style='width:55%'><table>{''.join(rows_html)}</table></td>"
            f"<td><table>{costs}</table><p class=small><b>Ship to</b> {e(ship.get('name'))}, {e(a)}<br>{e(o['email'])} · {e(ship.get('phone'))}"
            f"<br>{e((q.get('shipping') or {}).get('method'))}</p></td></tr></table>"
            f"<form method=post action='/admin/order/{e(o['id'])}' style='display:flex;gap:8px;margin-top:8px'>"
            f"<select name=status>{opts}</select><input name=tracking placeholder='tracking number' value='{e(o['tracking'])}'>"
            f"<input name=note placeholder=note value='{e(o['note'])}' style='flex:1'><button style='width:auto;padding:6px 16px'>Save</button></form></div>")
    return page("Orders", "".join(out))
