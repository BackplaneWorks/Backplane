# Order server

What Backplane's Buy it talks to (`src/core/order.bend`, `src/server/orderio.bend`).
A small Python service behind nginx on a Debian host.

## What it does

- `POST /v1/estimate`: the "from $X" on a project's card. Each part's kind, options and
  measured features (no files) priced bare at the smallest order, plus the fee.
- `POST /v1/quotes`: a buy thread's export. The hub sends the manifest and its files only
  after the person approved the list. The server measures the files itself
  (`analysis.py`, in a separate process under memory, CPU and file-size limits):
  - board size and layers from the gerbers, holes from the drill files;
  - pads from the paste layers, and BOM lines and placements;
  - volume, area and the tightest box from a STEP;
  - a flat pattern from a DXF.

  Then it prices what it measured (`pricing.py`). The client never sends a price.
- `GET /v1/quotes/{id}` with `Authorization: Bearer <token>`: the quote, and the order's progress.
- `/q/{id}?t=<token>`: the quote's Buy now link from Backplane. It redirects straight to a Stripe
  Checkout (an open one is reused) that collects card, email, phone and the shipping address in the
  quote's country, and shows the price-may-change terms beside Pay. There is no quote page: the quote is
  only shown in Backplane. Stripe comes back to `/q/{id}/done` or `/q/{id}/cancelled`, one line each.
- `/admin`: paid orders, to place by hand. Takes `?k=$FAB_ADMIN_TOKEN` once, which then
  becomes a cookie.

The person sees four lines and a total:
- manufacturing;
- shipping (the cheapest duty-paid express);
- duties and taxes (`duties.py`);
- the service and price-assurance fee: `FAB_FEE` (10%), and `FAB_FEE_CNC` (20%) on CNC parts.

Components come from the fab's own stock first: fab code or MPN, its attrition and
minimums. Mouser is used for the rest, marked up by `FAB_SOURCED_MARKUP` for the fab's
sourcing, with spare parts and the inspection and duty fees. A part found nowhere is a
problem to fix, not a guess.

## Security

- **Tokens:** every quote has an unguessable id and a separate 256-bit read token. Only the
  token's SHA-256 is stored. A wrong token and a missing quote both answer 404.
- **Rate limits:**
  - nginx limits request bursts;
  - `limits.py` sets hourly and daily budgets per address (an IPv6 /64 counts as one) and per
    install (`X-Backplane-Install`);
  - a global daily cap keeps distributor API quotas safe;
  - at most three quotes per install are being worked out at once.
- **Uploads:**
  - 50 MB at most;
  - only listed files are accepted, each matching its SHA-256;
  - only known file types;
  - plain relative paths only;
  - gerber archives unpacked with size, ratio and link checks;
  - nothing uploaded is ever run.
- **The service:**
  - runs as `fab` under systemd's sandboxing (read-only system, writes only to
    `/srv/fab/data`, no capabilities);
  - listens on 127.0.0.1 only;
  - TLS comes from Let's Encrypt, with short-lived certificates renewed by `fab-cert.timer`.
- **Payments:** confirmed by asking Stripe for the session, never by trusting the return
  URL, so no webhook endpoint is exposed.
- **Retention:** files of unpaid quotes are deleted after `FAB_KEEP_DAYS` (7).
- **Logs:** they hold no query strings, so tokens never land in them.

## Running it

The fab's price models and endpoints are a private pack, not in this repository. It holds
`vendor/` (implementing `fabsrv/vendor_api.py`) and the models it imports.

```sh
order/deploy/deploy.sh root@HOST /path/to/pack
```

First time on the host:
1. Install packages: `apt install nginx python3-venv libgl1 lego` (lego from backports).
2. Make the `fab` user.
3. Make `/srv/fab/{app,vendor,data}` and `/var/www/acme`.
4. Make the venv: `python3 -m venv /srv/fab/venv && /srv/fab/venv/bin/pip install -r requirements.txt`.
5. Fill `/etc/fab/env` (root:fab 0640):

   ```
   FAB_DATA=/srv/fab/data
   FAB_VENDOR=/srv/fab/vendor
   FAB_PUBLIC_URL=https://HOST
   FAB_ADMIN_TOKEN=...
   STRIPE_KEY=sk_...
   MOUSER_API_KEY=...
   ```

6. Get the certificate:
   `lego --accept-tos --disable-cn --path /etc/lego --domains HOST --http --http.webroot /var/www/acme run --profile shortlived`
7. Link it into `/etc/fab/tls/`.
8. Write `/etc/fab/cert.env` with `FAB_HOST=HOST`.

Locally: `FAB_DATA=... FAB_VENDOR=... uvicorn fabsrv.app:app --port 8090` from `order/`.

Tests: `pip install -r requirements.txt && python -m pytest tests`. They run against a stand-in fab pack
(`tests/fakevendor`): estimates, a quote measured from a small board's files, the upload checks
(paths, hashes, file types, zip links and bombs), tokens, and the address limits.
