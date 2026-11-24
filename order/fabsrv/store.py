"""SQLite: quotes and orders.

A quote's read token is shown once, in the reply that creates it; only its
SHA-256 is kept, so a copy of the database cannot read anyone's quote.
"""
import contextlib
import hashlib
import hmac
import json
import secrets
import sqlite3
import threading
import time

from . import config

_lock = threading.Lock()
_db = None

SCHEMA = """
create table if not exists quotes (
  id text primary key,
  token_hash text not null,
  install text not null,
  ip text not null,
  status text not null,            -- received | analysing | ready | failed | expired
  created real not null,
  expires real not null,
  country text not null default '',
  manifest text not null default '{}',
  result text not null default '{}',
  error text not null default ''
);
create index if not exists quotes_install on quotes(install, status);
create index if not exists quotes_created on quotes(created);
create table if not exists orders (
  id text primary key,
  quote_id text not null references quotes(id),
  session text not null default '',   -- Stripe Checkout Session
  status text not null,               -- checkout | paid | placed | shipped | delivered | refunded | cancelled
  amount integer not null default 0,  -- cents charged
  email text not null default '',
  shipping text not null default '{}',
  tracking text not null default '',
  note text not null default '',
  created real not null,
  updated real not null
);
create index if not exists orders_quote on orders(quote_id);
create table if not exists order_log (
  order_id text not null references orders(id),
  at real not null,
  by text not null default '',        -- admin | stripe
  status text not null default '',    -- the order as it was before the change
  tracking text not null default '',
  note text not null default '',
  new_status text not null default ''
);
create index if not exists order_log_order on order_log(order_id, at);
"""


def db():
    global _db
    if _db is None:
        config.DATA.mkdir(parents=True, exist_ok=True)
        _db = sqlite3.connect(config.DATA / "fab.db", check_same_thread=False, isolation_level=None)
        _db.row_factory = sqlite3.Row
        _db.execute("pragma journal_mode=wal")
        _db.execute("pragma foreign_keys=on")
        _db.executescript(SCHEMA)
    return _db


def q(sql, *args):
    with _lock:
        return db().execute(sql, args).fetchall()


def one(sql, *args):
    r = q(sql, *args)
    return r[0] if r else None


def new_id(prefix):
    # 128 random bits: ids are not secret, but they are never guessable either
    return prefix + secrets.token_urlsafe(16)


def hash_token(token):
    return hashlib.sha256(token.encode()).hexdigest()


def token_ok(row, token):
    return bool(token) and row is not None and hmac.compare_digest(row["token_hash"], hash_token(token))


def create_quote(install, ip, country, manifest):
    qid = new_id("q_")
    token = secrets.token_urlsafe(32)
    now = time.time()
    q("insert into quotes (id, token_hash, install, ip, status, created, expires, country, manifest) values (?,?,?,?,?,?,?,?,?)",
      qid, hash_token(token), install, ip, "received", now, now + config.QUOTE_DAYS * 86400, country, json.dumps(manifest))
    return qid, token


def quote(qid):
    return one("select * from quotes where id = ?", qid)


def set_quote(qid, **kv):
    if "result" in kv and not isinstance(kv["result"], str):
        kv["result"] = json.dumps(kv["result"])
    cols = ", ".join(f"{k} = ?" for k in kv)
    q(f"update quotes set {cols} where id = ?", *kv.values(), qid)


def unfinished(install):
    return one("select count(*) n from quotes where install = ? and status in ('received','analysing')", install)["n"]


def order_for(qid):
    return one("select * from orders where quote_id = ? order by created desc limit 1", qid)


def create_order(qid, session, amount):
    oid = new_id("o_")
    now = time.time()
    q("insert into orders (id, quote_id, session, status, amount, created, updated) values (?,?,?,?,?,?,?)",
      oid, qid, session, "checkout", amount, now, now)
    return oid


def set_order(oid, **kv):
    kv["updated"] = time.time()
    if "shipping" in kv and not isinstance(kv["shipping"], str):
        kv["shipping"] = json.dumps(kv["shipping"])
    cols = ", ".join(f"{k} = ?" for k in kv)
    q(f"update orders set {cols} where id = ?", *kv.values(), oid)


@contextlib.contextmanager
def _tx():
    """One transaction under the lock (the connection otherwise commits each statement)."""
    with _lock:
        c = db()
        c.execute("begin immediate")
        try:
            yield c
        except BaseException:
            c.execute("rollback")
            raise
        c.execute("commit")


def order(oid):
    return one("select * from orders where id = ?", oid)


def never_paid(o):
    """A checkout nobody paid (still open, or expired and cancelled): Stripe alone moves it."""
    return o["status"] in ("checkout", "cancelled") and not o["email"]


def change_order(oid, by, **kv):
    """Set an order's status, tracking or note, keeping what they were in order_log (for the
    order list's history and Undo). Nothing is written when nothing changes."""
    with _tx() as c:
        o = c.execute("select * from orders where id = ?", (oid,)).fetchone()
        if o is None:
            return False
        moved = any(str(o[k]) != str(v) for k, v in kv.items() if k in ("status", "tracking", "note"))
        if not moved and set(kv) <= {"status", "tracking", "note"}:
            return False
        now = time.time()
        if moved:
            c.execute("insert into order_log (order_id, at, by, status, tracking, note, new_status) values (?,?,?,?,?,?,?)",
                      (oid, now, by, o["status"], o["tracking"], o["note"], kv.get("status", o["status"])))
        kv["updated"] = now
        if "shipping" in kv and not isinstance(kv["shipping"], str):
            kv["shipping"] = json.dumps(kv["shipping"])
        c.execute(f"update orders set {', '.join(f'{k} = ?' for k in kv)} where id = ?", (*kv.values(), oid))
        return True


def order_log(oid):
    return q("select rowid, * from order_log where order_id = ? order by at desc, rowid desc", oid)


def logs():
    """Every order's changes, newest first, by order id."""
    out = {}
    for r in q("select rowid, * from order_log order by at desc, rowid desc"):
        out.setdefault(r["order_id"], []).append(r)
    return out


def undo(oid):
    """Put back what the newest change from the order list replaced, and forget that change.
    False when the newest change was not the order list's (Stripe's are never undone)."""
    with _tx() as c:
        r = c.execute("select rowid, * from order_log where order_id = ? order by at desc, rowid desc limit 1", (oid,)).fetchone()
        if r is None or r["by"] != "admin":
            return False
        c.execute("update orders set status = ?, tracking = ?, note = ?, updated = ? where id = ?",
                  (r["status"], r["tracking"], r["note"], time.time(), oid))
        c.execute("delete from order_log where rowid = ?", (r["rowid"],))
        return True


def orders(limit=1000):
    """Every order, open checkouts too, newest first, with its quote."""
    return q("select o.*, q.country, q.result, q.manifest from orders o join quotes q on q.id = o.quote_id "
             "order by o.created desc limit ?", limit)


def open_checkouts():
    return q("select * from orders where status = 'checkout' and created > ?", time.time() - 2 * 86400)


def stale_quotes(before):
    """Quotes past their keep date with no paid order: their files can go."""
    return q("select id from quotes where created < ? and id not in "
             "(select quote_id from orders where status not in ('checkout','cancelled'))", before)
