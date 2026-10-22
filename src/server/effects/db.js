// db.bend's effects over bun:sqlite, required lazily so a web or phone
// bundle that includes this file never touches it. Handles are small slots
// (the lowest free one from 1).
const bp_db_slots = [];
const BP_DB_SLOTS = 256;
let bp_db_lib = null;
function bp_db_sqlite() {
  if (!bp_db_lib) bp_db_lib = require("bun:sqlite");
  return bp_db_lib;
}
function bp_db_why(e) {
  return String(e && e.message ? e.message : e);
}
function bp_db_list(xs) {
  const out = [];
  for (; xs.$ === CID(Con); xs = xs.tail) out.push(xs.head);
  return out;
}
function bp_db_from(xs) {
  let out = { $: CID(Nil) };
  for (let i = xs.length - 1; i >= 0; i--) out = { $: CID(Con), head: xs[i], tail: out };
  return out;
}
// the database a handle names, or null
function bp_db_get(h) {
  if (!/^[0-9]+$/.test(h)) return null;
  const k = Number(h);
  return k > 0 && k < BP_DB_SLOTS ? bp_db_slots[k] || null : null;
}
// params as bound: a String list binds every one as text (never read for a
// type); a typed (DbVal) list says each: DbText, DbInt (decimal digits in
// int64's range, as a BigInt) or DbNull
const BP_DB_I64 = 9223372036854775807n;
function bp_db_val(v) {
  if (v.$ === CID(DbText)) return v.text;
  if (v.$ === CID(DbNull)) return null;
  if (v.$ === CID(DbInt) && /^-?[0-9]{1,19}$/.test(v.digits)) {
    const n = BigInt(v.digits);
    if (n <= BP_DB_I64 && n >= -BP_DB_I64 - 1n) return n;
  }
  throw new Error("not an integer: " + v.digits);
}
function bp_db_params(ps, typed) {
  const out = bp_db_list(ps);
  return typed ? out.map(bp_db_val) : out;
}
// a column as SQLite's own text for it (sqlite3_column_text): REAL as
// "%!.15g", integers in full, NULL as ""
function bp_db_real(x) {
  if (x !== x) return "";
  if (x === Infinity) return "Inf";
  if (x === -Infinity) return "-Inf";
  if (x === 0) return "0.0";
  const [m, ex] = x.toExponential(14).split("e");
  const n = Number(ex);
  const trim = (s) => {
    if (s.includes(".")) s = s.replace(/0+$/, "");
    return s.endsWith(".") ? s + "0" : s.includes(".") ? s : s + ".0";
  };
  if (n < -4 || n >= 15) {
    const a = Math.abs(n);
    return trim(m) + "e" + (n < 0 ? "-" : "+") + (a < 10 ? "0" : "") + a;
  }
  return trim(x.toFixed(14 - n));
}
function bp_db_text(v) {
  if (v === null || v === void 0) return "";
  if (typeof v === "bigint") return v.toString();
  if (typeof v === "number") return bp_db_real(v);
  if (v instanceof Uint8Array) return Buffer.from(v).toString("utf8");
  return String(v);
}
function db_open(path) {
  try {
    if (path.includes("\0")) return "fail a NUL in the path";
    const { Database } = bp_db_sqlite();
    const db = new Database(path, { create: true, readwrite: true, safeIntegers: true });
    try {
      db.exec("PRAGMA busy_timeout=5000");
      db.exec("PRAGMA journal_mode=WAL");
      db.exec("PRAGMA synchronous=NORMAL");
    } catch (e) {
      db.close();
      throw e;
    }
    let k = 1;
    while (k < BP_DB_SLOTS && bp_db_slots[k]) k++;
    if (k >= BP_DB_SLOTS) {
      db.close();
      return "fail too many handles";
    }
    bp_db_slots[k] = db;
    return "ok " + k;
  } catch (e) {
    return "fail " + bp_db_why(e);
  }
}
function db_exec(h, sql) {
  const db = bp_db_get(h);
  if (!db) return "fail no handle " + h;
  if (sql.includes("\0")) return "fail a NUL in the SQL";
  try {
    db.exec(sql);
    return "ok";
  } catch (e) {
    return "fail " + bp_db_why(e);
  }
}
// a statement, kept by bun:sqlite (query) or not (prepare); SQL with no
// statement in it fails as "no statement"
function bp_db_prep(db, sql, keep) {
  let st;
  try {
    st = keep ? db.query(sql) : db.prepare(sql);
  } catch (e) {
    const m = bp_db_why(e);
    // what Bun says for SQL that is empty or only comments (1.3: the first
    // two, 1.4: the last)
    if (m === "SQL query cannot be empty." || m === "Statement has finalized" || m === "Invalid SQL statement") throw new Error("no statement");
    throw e;
  }
  return st;
}
// one statement (bun:sqlite keeps it, prepared, by its SQL text) bound and
// run: the rows it changed
function bp_db_one(db, sql, params, typed) {
  const st = bp_db_prep(db, sql, true);
  const ps = bp_db_params(params, typed);
  if (st.paramsCount !== ps.length) throw new Error(ps.length + " params for " + st.paramsCount + " placeholders");
  // bun:sqlite's changes is how far total_changes moved (trigger and FTS5
  // writes included); the answer is the statement's own rows (sqlite3_changes)
  if (st.run(...ps).changes == 0) return 0;
  return Number(db.query("SELECT changes()").values()[0][0]);
}
function bp_db_run(h, sql, params, typed) {
  const db = bp_db_get(h);
  if (!db) return "fail no handle " + h;
  try {
    return "ok " + bp_db_one(db, sql, params, typed);
  } catch (e) {
    return "fail " + bp_db_why(e);
  }
}
// rows run one at a time inside a savepoint (so inside or outside the
// caller's own BEGIN), all or none; when sql is null (a script) each row is
// [sql, ...params], or a DbStmt when typed
function bp_db_rows(h, sql, rows, typed) {
  const db = bp_db_get(h);
  if (!db) return "fail no handle " + h;
  try {
    db.exec("SAVEPOINT bp_db");
  } catch (e) {
    return "fail " + bp_db_why(e);
  }
  try {
    let total = 0;
    for (let xs = rows; xs.$ === CID(Con); xs = xs.tail) {
      const row = xs.head;
      if (sql !== null) total += bp_db_one(db, sql, row, typed);
      else if (typed) total += bp_db_one(db, row.sql, row.params, true);
      else if (row.$ !== CID(Con)) throw new Error("an empty item");
      else total += bp_db_one(db, row.head, row.tail, false);
    }
    db.exec("RELEASE bp_db");
    return "ok " + total;
  } catch (e) {
    try {
      db.exec("ROLLBACK TO bp_db; RELEASE bp_db");
    } catch {}
    return "fail " + bp_db_why(e);
  }
}
function db_run(h, sql, params) {
  return bp_db_run(h, sql, params, false);
}
function db_run_t(h, sql, params) {
  return bp_db_run(h, sql, params, true);
}
function db_batch(h, sql, rows) {
  return bp_db_rows(h, sql, rows, false);
}
function db_batch_t(h, sql, rows) {
  return bp_db_rows(h, sql, rows, true);
}
function db_script(h, items) {
  return bp_db_rows(h, null, items, false);
}
// Builder-only status path. Keep the counted APIs above unchanged. Reuse
// their preparation and String parameter helpers and identical savepoint
// protocol; a write's result count is deliberately never queried.
function db_script_status(h, items) {
  const db = bp_db_get(h);
  if (!db) return "fail no handle " + h;
  try {
    db.exec("SAVEPOINT bp_db");
  } catch (e) {
    return "fail " + bp_db_why(e);
  }
  try {
    for (let xs = items; xs.$ === CID(Con); xs = xs.tail) {
      const row = xs.head;
      if (row.$ !== CID(Con)) throw new Error("an empty item");
      const st = bp_db_prep(db, row.head, true);
      const ps = bp_db_params(row.tail, false);
      if (st.paramsCount !== ps.length) throw new Error(ps.length + " params for " + st.paramsCount + " placeholders");
      st.run(...ps);
    }
    db.exec("RELEASE bp_db");
    return "ok";
  } catch (e) {
    try {
      db.exec("ROLLBACK TO bp_db; RELEASE bp_db");
    } catch {}
    return "fail " + bp_db_why(e);
  }
}
function db_script_t(h, items) {
  return bp_db_rows(h, null, items, true);
}
function bp_db_fail(why) {
  return bp_db_from([bp_db_from(["\0fail", why])]);
}
function bp_db_query(h, sql, params, limit, typed) {
  const db = bp_db_get(h);
  if (!db) return bp_db_fail("no handle " + h);
  let st;
  try {
    st = bp_db_prep(db, sql, false);
    const ps = bp_db_params(params, typed);
    if (st.paramsCount !== ps.length) throw new Error(ps.length + " params for " + st.paramsCount + " placeholders");
    const out = [];
    if (limit > 0) {
      // rows as objects only when no name repeats: objects keep one of
      // each (Bun 1.3's columnNames does too; native.columnsCount, where
      // Bun has it, counts every column)
      const names = st.columnNames;
      const cols = st.native && typeof st.native.columnsCount === "number" ? st.native.columnsCount : -1;
      if (cols === names.length && new Set(names).size === names.length) {
        // rows one at a time, stopping at the limit (keys as named)
        for (const r of st.iterate(...ps)) {
          out.push(bp_db_from(names.map((n) => bp_db_text(r[n]))));
          if (out.length >= limit) break;
        }
      } else {
        // repeated column names: objects would merge them, so whole rows
        for (const r of st.values(...ps)) {
          out.push(bp_db_from(r.map(bp_db_text)));
          if (out.length >= limit) break;
        }
      }
    }
    return bp_db_from(out);
  } catch (e) {
    return bp_db_fail(bp_db_why(e));
  } finally {
    if (st) st.finalize();
  }
}
function db_query(h, sql, params, limit) {
  return bp_db_query(h, sql, params, limit, false);
}
function db_query_t(h, sql, params, limit) {
  return bp_db_query(h, sql, params, limit, true);
}
function db_close(h) {
  const db = bp_db_get(h);
  if (!db) return "fail no handle " + h;
  try {
    db.close();
    return "ok";
  } catch (e) {
    return "fail " + bp_db_why(e);
  } finally {
    bp_db_slots[Number(h)] = null;
  }
}
io_eff(CID(Db.open), db_open);
io_eff(CID(Db.exec), db_exec);
io_eff(CID(Db.run), db_run);
io_eff(CID(Db.batch), db_batch);
io_eff(CID(Db.script), db_script);
io_eff(CID(Db.script_status), db_script_status);
io_eff(CID(Db.query), db_query);
io_eff(CID(Db.close), db_close);
io_eff(CID(Db.run.t), db_run_t);
io_eff(CID(Db.batch.t), db_batch_t);
io_eff(CID(Db.script.t), db_script_t);
io_eff(CID(Db.query.t), db_query_t);
