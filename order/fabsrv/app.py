"""HTTP API and pages.

    GET  /v1/health                     is the server up, and its catalog version
    GET  /v1/catalog                    what can be ordered (catalog.py)
    POST /v1/estimate                   part features -> "from $X" (no files)
    POST /v1/quotes                     manifest + files -> {id, token}
    GET  /v1/quotes/{id}                (Bearer token) status, the quote, the order's status
    GET  /q/{id}?t=token                Backplane's Buy now: straight to Stripe Checkout (no quote page)
    GET  /q/{id}/done?s=session         back from Stripe: payment confirmed with Stripe itself
    GET  /q/{id}/cancelled              back from Stripe without paying
    GET  /admin                         paid orders (FAB_ADMIN_TOKEN as a cookie or ?k=)
    GET  /admin/files/{id}              a paid quote's files as one zip
    POST /admin/order/{id}              an order's status, tracking and note

Every client sends X-Backplane-Install, a random id made once per install;
limits apply to it and to the address (limits.py).
"""
import asyncio
import contextlib
import hmac
import io
import json
import re
import shutil
import time
import zipfile

from fastapi import FastAPI, HTTPException, Request
from fastapi.responses import HTMLResponse, JSONResponse, RedirectResponse, Response

from . import catalog, config, estimate, limits, package, pages, pay, quoting, store, vendor_api

@contextlib.asynccontextmanager
async def lifespan(_app):
    store.db()
    quoting.start()
    task = asyncio.get_running_loop().create_task(housekeeping())
    yield
    task.cancel()


app = FastAPI(docs_url=None, redoc_url=None, openapi_url=None, lifespan=lifespan)
INSTALL = re.compile(r"^[A-Za-z0-9_-]{16,64}$")
QID = re.compile(r"^q_[A-Za-z0-9_-]{20,24}$")


@app.middleware("http")
async def headers(request: Request, call_next):
    resp = await call_next(request)
    resp.headers["X-Content-Type-Options"] = "nosniff"
    resp.headers["Referrer-Policy"] = "no-referrer"
    resp.headers["X-Frame-Options"] = "DENY"
    resp.headers["Cache-Control"] = "no-store"
    if resp.headers.get("content-type", "").startswith("text/html"):
        resp.headers["Content-Security-Policy"] = (
            "default-src 'none'; style-src 'unsafe-inline'; img-src 'self' data:; form-action 'self' https://checkout.stripe.com; "
            "base-uri 'none'; frame-ancestors 'none'")
    return resp


def client_ip(request: Request):
    peer = request.client.host if request.client else ""
    if peer in config.TRUSTED_PROXIES:
        return request.headers.get("x-real-ip", peer)
    return peer


def install_of(request: Request):
    i = request.headers.get("x-backplane-install", "")
    if not INSTALL.match(i):
        raise HTTPException(400, "send X-Backplane-Install: a random id (16-64 of A-Z a-z 0-9 _ -) made once per install")
    return i


def limited(scope, request, install=""):
    wait = limits.check(scope, client_ip(request), install)
    if wait:
        raise HTTPException(429, f"too many requests; try again in {wait // 60 + 1} min",
                            headers={"Retry-After": str(wait)})


async def body_json(request: Request, cap):
    n = request.headers.get("content-length")
    if n is None or not n.isdigit() or int(n) > cap:
        raise HTTPException(413, "request too large")
    raw = await request.body()
    if len(raw) > cap:
        raise HTTPException(413, "request too large")
    try:
        return json.loads(raw)
    except ValueError:
        raise HTTPException(400, "the body must be JSON")


@app.exception_handler(HTTPException)
async def http_error(request: Request, exc: HTTPException):
    if request.url.path.startswith("/v1/"):
        return JSONResponse({"error": exc.detail}, status_code=exc.status_code, headers=exc.headers)
    return HTMLResponse(pages.error(exc.status_code, str(exc.detail)), status_code=exc.status_code, headers=exc.headers)


@app.get("/v1/health")
async def health():
    return {"ok": True, "version": config.VERSION, "pricing": vendor_api.available()}


@app.get("/v1/catalog")
async def get_catalog(request: Request):
    limited("read", request)
    return {"version": config.VERSION, "kinds": catalog.schema(), "fee": config.FEE, "fee_cnc": config.FEE_CNC,
            "min_assembled": catalog.MIN_ASSEMBLED}


@app.post("/v1/estimate")
async def post_estimate(request: Request):
    install = install_of(request)
    limited("estimate", request, install)
    body = await body_json(request, 64 * 1024)
    if not isinstance(body, dict) or not isinstance(body.get("parts"), list):
        raise HTTPException(400, "send {parts: [{name, kind, options, features}]}")
    try:
        return await asyncio.to_thread(estimate.estimate, body["parts"])
    except estimate.Unavailable:
        raise HTTPException(503, "pricing is not available right now")


@app.post("/v1/quotes")
async def post_quote(request: Request):
    install = install_of(request)
    n = request.headers.get("content-length")
    if n is None or not n.isdigit() or int(n) > config.MAX_UPLOAD + 1024 * 1024:
        raise HTTPException(413, f"the package must be under {config.MAX_UPLOAD // (1024 * 1024)} MB")
    if store.unfinished(install) >= 3:
        raise HTTPException(429, "three quotes are already being worked out; wait for one to finish")
    limited("quote", request, install)
    form = await request.form(max_files=config.MAX_FILES, max_fields=4, max_part_size=256 * 1024)
    try:
        try:
            manifest = package.check_manifest(json.loads(form.get("manifest") or ""))
        except ValueError:
            raise package.Bad("the manifest must be JSON")
        uploads = []
        for key, v in form.multi_items():
            if key == "file":
                if not hasattr(v, "read"):
                    raise package.Bad("file parts must be files")
                uploads.append((v.filename or "", await v.read()))
            elif key != "manifest":
                raise package.Bad(f"unexpected field {key}")
        qid, token = store.create_quote(install, client_ip(request), manifest["ship_to"], manifest)
        root = config.DATA / "quotes" / qid
        try:
            await asyncio.to_thread(package.store_files, manifest, uploads, root)
        except BaseException:
            shutil.rmtree(root, ignore_errors=True)
            store.set_quote(qid, status="failed", error="refused")
            raise
    except package.Bad as e:
        raise HTTPException(400, str(e))
    finally:
        await form.close()
    quoting.submit(qid)
    return JSONResponse({"id": qid, "token": token, "status": "received", "poll_after": 3}, status_code=202)


def quote_for(qid, token):
    row = store.quote(qid) if QID.match(qid or "") else None
    if not store.token_ok(row, token):
        # one answer for "no such quote" and "wrong token": nothing tells whether a quote exists
        raise HTTPException(404, "no such quote")
    return row


def bearer(request: Request):
    h = request.headers.get("authorization", "")
    return h[7:] if h.lower().startswith("bearer ") else ""


def public_quote(row):
    out = {"id": row["id"], "status": row["status"], "expires": int(row["expires"]), "country": row["country"], "pilot": config.PILOT}
    if row["status"] == "ready":
        out["quote"] = json.loads(row["result"])
        # what each part costs stays with us (the order list shows it)
        out["quote"].pop("part_lines", None)
        out["pay_path"] = f"/q/{row['id']}"
        if time.time() > row["expires"]:
            out["status"] = "expired"
    if row["status"] == "failed":
        out["error"] = row["error"] or "the quote could not be worked out"
    o = store.order_for(row["id"])
    if o is not None and o["status"] != "checkout":
        out["order"] = {"id": o["id"], "status": o["status"], "amount": o["amount"] / 100, "tracking": o["tracking"],
                        "updated": int(o["updated"])}
    return out


@app.get("/v1/quotes/{qid}")
async def get_quote(qid: str, request: Request):
    limited("read", request)
    return public_quote(quote_for(qid, bearer(request)))


@app.get("/q/{qid}")
async def quote_buy(qid: str, request: Request, t: str = ""):
    """A quote's Buy now link from Backplane: straight to Stripe Checkout. The quote
    itself is only ever shown in Backplane; there is no page of it here."""
    limited("pay", request)
    row = quote_for(qid, t)
    q = public_quote(row)
    o = q.get("order")
    if o and o["status"] not in ("cancelled", "checkout"):
        return HTMLResponse(pages.done(o["status"]))
    if q["status"] != "ready":
        return HTMLResponse(pages.error("Not payable", "This quote can no longer be paid. Ask for a new one in Backplane."), status_code=409)
    if q["quote"].get("problems"):
        return HTMLResponse(pages.error("Not payable", "This quote has problems to fix first. See the thread in Backplane."), status_code=409)
    try:
        url = await asyncio.to_thread(pay.checkout, row, q["quote"])
    except pay.Unavailable as e:
        return HTMLResponse(pages.error("Payments are unavailable", str(e)), status_code=503)
    return RedirectResponse(url, status_code=303)


@app.get("/q/{qid}/done", response_class=HTMLResponse)
async def quote_done(qid: str, request: Request, s: str = ""):
    limited("read", request)
    row = store.quote(qid) if QID.match(qid) else None
    o = store.order_for(qid) if row else None
    if o is None or not s or not hmac.compare_digest(o["session"], s):
        raise HTTPException(404, "no such order")
    await asyncio.to_thread(pay.confirm, o)
    o = store.order_for(qid)
    return pages.done(o["status"])


@app.get("/q/{qid}/cancelled", response_class=HTMLResponse)
async def quote_cancelled(qid: str):
    return pages.cancelled()


def admin_ok(request: Request, k=""):
    tok = k or request.cookies.get("fab_admin", "")
    return bool(config.ADMIN_TOKEN) and bool(tok) and hmac.compare_digest(tok, config.ADMIN_TOKEN)


@app.get("/admin", response_class=HTMLResponse)
async def admin(request: Request, k: str = ""):
    limited("pay", request)
    if not admin_ok(request, k):
        raise HTTPException(403, "forbidden")
    resp = HTMLResponse(pages.admin(store.orders()))
    if k:
        resp.set_cookie("fab_admin", k, httponly=True, secure=config.PUBLIC_URL.startswith("https"), samesite="strict",
                        max_age=7 * 86400)
    return resp


@app.get("/admin/files/{qid}")
async def admin_files(qid: str, request: Request):
    """A paid quote's files as one zip, to hand to the fab (unpacked gerber copies left out)."""
    limited("pay", request)
    if not admin_ok(request):
        raise HTTPException(403, "forbidden")
    if not QID.match(qid):
        raise HTTPException(404, "not found")
    root = config.DATA / "quotes" / qid / "files"
    if not root.is_dir():
        raise HTTPException(404, "the files are gone")
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for f in sorted(root.rglob("*")):
            rel = f.relative_to(root)
            if f.is_file() and not any(part.endswith(".d") for part in rel.parts[:-1]):
                z.write(f, str(rel))
        z.write(config.DATA / "quotes" / qid / "manifest.json", "manifest.json")
    return Response(buf.getvalue(), media_type="application/zip",
                    headers={"Content-Disposition": f"attachment; filename={qid}.zip"})


@app.post("/admin/order/{oid}")
async def admin_order(oid: str, request: Request):
    limited("pay", request)
    if not admin_ok(request):
        raise HTTPException(403, "forbidden")
    form = await request.form(max_files=0, max_fields=4, max_part_size=4096)
    status = str(form.get("status") or "")
    if status not in ("paid", "placed", "shipped", "delivered", "refunded", "cancelled"):
        raise HTTPException(400, "bad status")
    store.set_order(oid, status=status, tracking=str(form.get("tracking") or "")[:200],
                    note=str(form.get("note") or "")[:2000])
    return RedirectResponse("/admin", status_code=303)


async def housekeeping():
    """Every ten minutes: confirm open checkouts with Stripe, forget idle rate-limit windows,
    and delete the files of quotes nobody paid for."""
    while True:
        try:
            await asyncio.to_thread(pay.reconcile)
            limits.sweep()
            await asyncio.to_thread(quoting.clean_old)
        except Exception as e:  # keep the loop alive; the log says what went wrong
            print("housekeeping:", repr(e), flush=True)
        await asyncio.sleep(600)
