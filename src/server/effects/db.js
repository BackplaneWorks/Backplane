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
// Read results, remembered until this process next writes what they read
// or opens/closes a database. A view renders its thread's subagent and
// context history from windowed queries on every request
// (subagent-history.bend: 20-60 ms on a thread with many subagents, every
// open, action and update; 2026-11-05), while nothing they read was written
// in between. store.db has no other writer (one hub per home; the other
// readers open it read-only), so an answer is always what the database
// holds now:
// - a write whose statements all name their table (INSERT/REPLACE INTO,
//   UPDATE, DELETE FROM a plain or main. name: run, batch, script and their
//   typed forms) drops the answers that read, by name (bp_db_tables), that
//   table or what the schema derives from it (views over it, an FTS table
//   kept from it, tables its triggers write, tables referencing it);
// - a read whose every use of a table T is "FROM T WHERE c = ?1" followed by
//   AND or the clause's end (no OR beside it), ?1 a text, c a TEXT column
//   with no collation, is about one value of c: a plain INSERT INTO T(...)
//   VALUES (...) [ON CONFLICT DO NOTHING] of rows with other values of c
//   keeps it, unless T's triggers write T (rows of another thread are not
//   rows this read can see; streaming bots insert their own threads' entry
//   rows while a view reads its thread's);
// - any other write (exec, a CTE-led or qualified statement, DDL) and every
//   open/close drops all of them.
// Only plain reads are kept: SELECT/WITH with no write and no clock or random
// function, at most 128 answers and 64 MiB of text (the least recently used
// go first; an answer over 32 MiB is not kept); failures are never kept.
// (Streaming bots write meta, setting, line, obj and entry rows many times
// a second: with every write dropping everything, 0 of ~1,400 reads hit on
// the huge state with 20 bots streaming, and the shown thread's subagent
// history ran on every render, ~85 ms there; that answer is 8-21 MiB on its
// two longest threads, 12-29k rows, so the caps were 1 and 16 MiB in vain;
// 2026-11-05.)
const bp_db_reads = new Map();
const BP_DB_READS = 128, BP_DB_READ_BYTES = 64 << 20, BP_DB_READ_MAX = 32 << 20;
let bp_db_read_bytes = 0;
// per handle, from sqlite_master: table -> the names a write to it changes
// (itself included), and the tables whose triggers write themselves; plus
// each table's TEXT columns with no collation. Dropped with the reads on
// exec, DDL and open/close.
const bp_db_derived = new Map();
function bp_db_wrote() {
  if (bp_db_reads.size) bp_db_reads.clear();
  bp_db_read_bytes = 0;
  bp_db_derived.clear();
}
const BP_DB_WORD = /[A-Za-z_][A-Za-z0-9_]*/g;
function bp_db_words(sql) {
  const ws = new Set();
  for (const w of sql.match(BP_DB_WORD) || []) ws.add(w.toLowerCase());
  return ws;
}
// the tables (and views, CTEs, table functions) a read names where a table
// goes: FROM and JOIN lists (comma joins, aliases, schema-qualified and
// quoted names, subqueries' own FROMs found in turn) and "x IN table". A
// SELECT reads tables nowhere else; a column that shares a table's name
// (entry.thread, the thread table) is not one.
const BP_DB_STOP = new Set(["WHERE", "ON", "USING", "JOIN", "LEFT", "RIGHT", "FULL", "INNER", "OUTER", "CROSS", "NATURAL",
  "GROUP", "ORDER", "LIMIT", "UNION", "EXCEPT", "INTERSECT", "WINDOW", "HAVING", "INDEXED", "NOT", "AS", "SELECT", "VALUES"]);
function bp_db_tables(sql) {
  const ts = new Set(), n = sql.length;
  const ws = (k) => { while (k < n && /\s/.test(sql[k])) k++; return k; };
  const parens = (k) => {
    for (let depth = 0; k < n; k++) {
      const ch = sql[k];
      if (ch === "\x27" || ch === "\x22" || ch === "\x60") { const e = sql.indexOf(ch, k + 1); if (e < 0) return n; k = e; }
      else if (ch === "(") depth++;
      else if (ch === ")" && --depth === 0) return k + 1;
    }
    return n;
  };
  const ident = (k) => {
    const q = sql[k], close = q === "[" ? "]" : q;
    if (q === "\x22" || q === "\x60" || q === "[") { const e = sql.indexOf(close, k + 1); return e < 0 ? null : { name: sql.slice(k + 1, e), end: e + 1, quoted: true }; }
    const m = /^[A-Za-z_][A-Za-z0-9_$]*/.exec(sql.slice(k, k + 256));
    return m ? { name: m[0], end: k + m[0].length, quoted: false } : null;
  };
  for (const m of sql.matchAll(/\b(FROM|JOIN|IN)\b/gi)) {
    const isIn = m[1].toUpperCase() === "IN";
    let k = m.index + m[0].length;
    for (;;) {
      k = ws(k);
      if (sql[k] === "(") {
        if (isIn) break;
        k = parens(k);
      } else {
        let id = ident(k);
        if (!id || !id.quoted && BP_DB_STOP.has(id.name.toUpperCase())) break;
        k = id.end;
        if (sql[k] === ".") {
          id = ident(k + 1);
          if (!id) break;
          k = id.end;
        }
        ts.add(id.name.toLowerCase());
        if (isIn) break;
        k = ws(k);
        if (sql[k] === "(") k = parens(k);
      }
      k = ws(k);
      let a = ident(k);
      if (a && !a.quoted && a.name.toUpperCase() === "AS") { k = ws(a.end); a = ident(k); }
      if (a && (a.quoted || !BP_DB_STOP.has(a.name.toUpperCase()))) { k = ws(a.end); a = ident(k); }
      // "INDEXED BY i" / "NOT INDEXED" after a table or its alias
      if (a && !a.quoted && a.name.toUpperCase() === "INDEXED") {
        const by = ident(ws(a.end)), i = by ? ident(ws(by.end)) : null;
        if (!i) break;
        k = ws(i.end);
      } else if (a && !a.quoted && a.name.toUpperCase() === "NOT") {
        const ix = ident(ws(a.end));
        if (ix && ix.name.toUpperCase() === "INDEXED") k = ws(ix.end);
      }
      if (sql[k] !== ",") break;
      k++;
    }
  }
  return ts;
}
// the table one write statement changes, or null (not known: drop all)
const BP_DB_TARGET = /^\s*(?:INSERT(?:\s+OR\s+[A-Za-z]+)?\s+INTO|REPLACE\s+INTO|UPDATE(?:\s+OR\s+[A-Za-z]+)?|DELETE\s+FROM)\s+(?:main\.)?([A-Za-z_][A-Za-z0-9_]*)(?=[\s(]|$)/i;
function bp_db_target(sql) {
  if (typeof sql !== "string") return null;
  const m = BP_DB_TARGET.exec(sql);
  return m ? m[1].toLowerCase() : null;
}
// a plain insert's columns and how each takes its parameter ("?" as is,
// "substr(?, 2)" without its first character), or null
const BP_DB_INSERT = /^\s*INSERT\s+INTO\s+(?:main\.)?([A-Za-z_][A-Za-z0-9_]*)\s*\(([^()]*)\)\s*VALUES\s*\(((?:\s*(?:\?|substr\(\?,\s*2\))\s*,)*\s*(?:\?|substr\(\?,\s*2\))\s*)\)\s*(?:ON\s+CONFLICT\s+DO\s+NOTHING\s*)?;?\s*$/i;
const bp_db_inserts = new Map();
function bp_db_insert(sql) {
  let r = bp_db_inserts.get(sql);
  if (r !== undefined) return r;
  const m = BP_DB_INSERT.exec(sql);
  r = null;
  if (m) {
    const cols = m[2].split(",").map((c) => c.trim().toLowerCase()), marks = m[3].split(/,(?![^(]*\))/).map((x) => x.trim() !== "?");
    if (cols.length === marks.length && cols.every((c) => /^[a-z_][a-z0-9_]*$/.test(c))) r = { table: m[1].toLowerCase(), cols, strip: marks };
  }
  if (bp_db_inserts.size >= 256) bp_db_inserts.clear();
  bp_db_inserts.set(sql, r);
  return r;
}
// a parameter as the text SQLite compares, or null (not a plain text)
function bp_db_param_text(v, typed) {
  if (typed) return v && v.$ === CID(DbText) ? v.text : null;
  return typeof v === "string" ? v : null;
}
// the one value a read is about: { table, col, value } or null
function bp_db_scope(sql, params, typed) {
  if (params.$ !== CID(Con)) return null;
  const value = bp_db_param_text(params.head, typed);
  if (value === null) return null;
  const uses = new Map();
  for (const m of sql.matchAll(/\bFROM\s+([A-Za-z_][A-Za-z0-9_]*)\s+WHERE\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\?1(?![0-9])/gi)) {
    if (!bp_db_conjunct(sql, m.index + m[0].length)) continue;
    const t = m[1].toLowerCase(), c = m[2].toLowerCase(), u = uses.get(t);
    if (u === undefined) uses.set(t, { col: c, n: 1 });
    else if (u.col !== c) uses.set(t, { col: null, n: 0 });
    else u.n++;
  }
  let scope = null;
  for (const [table, u] of uses) {
    if (u.col === null) continue;
    let n = 0;
    for (const w of sql.match(BP_DB_WORD) || []) if (w.toLowerCase() === table) n++;
    // (the table named anywhere else: not about one value)
    if (n !== u.n) continue;
    if (scope !== null) return null;
    scope = { table, col: u.col, value };
  }
  return scope;
}
// after "c = ?1" at i: the rest of its WHERE clause, to the clause's end
// (a closing parenthesis, ORDER/GROUP/LIMIT/..., or the end), joins it with
// AND only (no OR beside it at its own depth, no COLLATE on it)
function bp_db_conjunct(sql, i) {
  const next = /^\s*([A-Za-z_]+|\)|;|$)/.exec(sql.slice(i));
  if (!next || !/^(AND|ORDER|GROUP|LIMIT|UNION|EXCEPT|INTERSECT|WINDOW|HAVING|\)|;|)$/i.test(next[1])) return false;
  let depth = 0;
  for (let k = i; k < sql.length; k++) {
    const ch = sql[k];
    if (ch === "\x27" || ch === "\x22" || ch === "\x60") {
      const e = sql.indexOf(ch, k + 1);
      if (e < 0) return false;
      k = e;
    } else if (ch === "[") {
      const e = sql.indexOf("]", k + 1);
      if (e < 0) return false;
      k = e;
    } else if (ch === "(") depth++;
    else if (ch === ")") {
      if (depth === 0) return true;
      depth--;
    } else if (depth === 0 && /[A-Za-z_]/.test(ch) && (k === 0 || !/[A-Za-z0-9_]/.test(sql[k - 1]))) {
      const w = /^[A-Za-z_][A-Za-z0-9_]*/.exec(sql.slice(k))[0].toUpperCase();
      if (w === "OR") return false;
      if (["ORDER", "GROUP", "LIMIT", "UNION", "EXCEPT", "INTERSECT", "WINDOW", "HAVING"].includes(w)) return true;
      k += w.length - 1;
    }
  }
  return true;
}
// the schema's derivations for h, or null when it cannot be read
function bp_db_schema(h, db) {
  let d = bp_db_derived.get(h);
  if (d !== undefined) return d;
  const edges = new Map(), names = [], fires = new Map(), text = new Map(), edge = (a, b) => {
    if (a === b) return;
    if (!edges.has(a)) edges.set(a, new Set());
    edges.get(a).add(b);
  };
  try {
    for (const master of ["sqlite_master", "sqlite_temp_master"]) {
      for (const [type, name, tbl, sql] of db.query("SELECT type, name, tbl_name, sql FROM " + master + " WHERE sql IS NOT NULL").values()) {
        const n = String(name).toLowerCase(), t = String(tbl).toLowerCase(), ddl = String(sql);
        if (type === "view") for (const w of bp_db_words(ddl)) edge(w, n);
        // (every word a trigger's body names: whatever it writes)
        else if (type === "trigger") {
          for (const w of bp_db_words(ddl)) edge(t, w);
          if (!fires.has(t)) fires.set(t, new Set());
          for (const w of bp_db_words(ddl)) fires.get(t).add(w);
        } else if (type === "table") {
          for (const m of ddl.matchAll(/\b(?:REFERENCES|content)\s*=?\s*[^\w\s]?([A-Za-z_][A-Za-z0-9_]*)/gi)) edge(m[1].toLowerCase(), n);
          names.push(n);
          // TEXT columns with no collation anywhere in the table (no quote
          // characters in these regular expressions: the build's effect
          // scanner would take one for a string's start)
          if (!/\bCOLLATE\b/i.test(ddl)) {
            const cs = new Set();
            for (const m of ddl.matchAll(/[(,]\s*([A-Za-z_][A-Za-z0-9_]*)\s+TEXT\b/gi)) cs.add(m[1].toLowerCase());
            text.set(n, cs);
          }
        }
      }
    }
    // a virtual table's shadow tables (entry_fts_data, ...) change with it
    for (const v of names) for (const x of names) if (x.startsWith(v + "_")) edge(v, x);
  } catch {
    d = null;
    bp_db_derived.set(h, d);
    return d;
  }
  const closure = new Map(), close = (a) => {
    let seen = closure.get(a);
    if (seen !== undefined) return seen;
    seen = new Set([a]);
    const todo = [a];
    while (todo.length) for (const b of edges.get(todo.pop()) || []) if (!seen.has(b)) { seen.add(b); todo.push(b); }
    closure.set(a, seen);
    return seen;
  };
  // a table whose triggers, followed through, write it again
  const self = new Set();
  for (const [t, ws] of fires) for (const w of ws) if (close(w).has(t)) self.add(t);
  d = { close, self, text };
  bp_db_derived.set(h, d);
  return d;
}
// statements about to run on h ({ sql, params } each): drop the reads they
// can change (all, when one is not known)
function bp_db_writes(h, stmts, typed) {
  const ts = [];
  for (const st of stmts) {
    const t = bp_db_target(st.sql);
    // (DDL among them: the schema too)
    if (t === null) return bp_db_wrote();
    ts.push(t);
  }
  if (!bp_db_reads.size) return;
  const db = bp_db_get(h);
  const d = db ? bp_db_schema(h, db) : null;
  if (d === null) return bp_db_wrote();
  stmts.forEach((st, i) => {
    const names = d.close(ts[i]), ins = d.self.has(ts[i]) ? null : bp_db_insert(st.sql);
    for (const [key, kept] of bp_db_reads) {
      let hit = false, other = false;
      for (const n of names) if (kept.tables.has(n)) {
        hit = true;
        if (n !== ts[i]) other = true;
      }
      if (!hit) continue;
      const sc = kept.scope;
      if (!other && sc !== null && ins !== null && sc.table === ins.table && d.text.has(sc.table) && d.text.get(sc.table).has(sc.col)) {
        const c = ins.cols.indexOf(sc.col);
        if (c >= 0 && st.params.every((ps) => {
          let v = ps;
          for (let k = 0; k < c && v.$ === CID(Con); k++) v = v.tail;
          if (v.$ !== CID(Con)) return false;
          let x = bp_db_param_text(v.head, typed);
          if (x === null || ins.strip[c] && (x.length === 0 || x.charCodeAt(0) >= 0xd800 && x.charCodeAt(0) < 0xdc00)) return false;
          if (ins.strip[c]) x = x.slice(1);
          return x !== sc.value;
        })) continue;
      }
      bp_db_reads.delete(key);
      bp_db_read_bytes -= kept.size;
    }
  });
}
// the text an answer holds (UTF-16 units, two bytes each), up to the cap
function bp_db_size(rows) {
  let n = 0;
  for (let r = rows; r.$ === CID(Con); r = r.tail)
    for (let c = r.head; c.$ === CID(Con); c = c.tail) {
      n += 16 + 2 * c.head.length;
      if (n > BP_DB_READ_MAX) return n;
    }
  return n;
}
function bp_db_pure(sql) {
  return /^\s*(SELECT|WITH)\b/i.test(sql) && !/\b(INSERT|UPDATE|DELETE|REPLACE|UPSERT|CREATE|DROP|ALTER|PRAGMA|ATTACH|DETACH|VACUUM|RANDOM|RANDOMBLOB|CHANGES|TOTAL_CHANGES|LAST_INSERT_ROWID|CURRENT_TIME|CURRENT_DATE|CURRENT_TIMESTAMP)\b|'now'/i.test(sql);
}
function bp_db_read_key(h, sql, params, limit, typed) {
  const ps = [];
  for (let xs = params; xs.$ === CID(Con); xs = xs.tail) {
    const v = xs.head;
    ps.push(typed ? (v.$ === CID(DbText) ? "t" + v.text : v.$ === CID(DbNull) ? "n" : "i" + v.digits) : "t" + v);
  }
  return JSON.stringify([h, typed, Number(limit), sql, ps]);
}
function db_open(path) {
  bp_db_wrote();
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
  bp_db_wrote();
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
  bp_db_writes(h, [{ sql, params: [params] }], typed);
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
  const stmts = [];
  if (sql !== null) stmts.push({ sql, params: bp_db_list(rows) });
  else for (let xs = rows; xs.$ === CID(Con); xs = xs.tail) {
    const row = xs.head;
    stmts.push(typed ? { sql: row.sql, params: [row.params] } : row.$ === CID(Con) ? { sql: row.head, params: [row.tail] } : { sql: null, params: [] });
  }
  bp_db_writes(h, stmts, typed);
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
// a kept answer, put back at the end (most recently used), or undefined
function bp_db_kept(key) {
  const kept = bp_db_reads.get(key);
  if (kept === undefined) return undefined;
  bp_db_reads.delete(key);
  bp_db_reads.set(key, kept);
  return kept.rows;
}
// keep an answer of size (bp_db_size) that reads tables (and scope, or null)
function bp_db_keep(key, rows, size, tables, scope) {
  if (size > BP_DB_READ_MAX) return;
  // over a cap, the answers used longest ago go first (a Map keeps its
  // insertion order, and a hit is put back at the end): the huge state's
  // big thread filled the cap in a few renders and emptied it every time
  for (const [k, old] of bp_db_reads) {
    if (bp_db_reads.size < BP_DB_READS && bp_db_read_bytes + size <= BP_DB_READ_BYTES) break;
    bp_db_reads.delete(k);
    bp_db_read_bytes -= old.size;
  }
  bp_db_reads.set(key, { rows, size, tables, scope });
  bp_db_read_bytes += size;
}
function bp_db_read(h, sql, params, limit, typed) {
  if (!bp_db_pure(sql)) return bp_db_query(h, sql, params, limit, typed);
  const key = bp_db_read_key(h, sql, params, limit, typed);
  const kept = bp_db_kept(key);
  if (kept !== undefined) return kept;
  const rows = bp_db_query(h, sql, params, limit, typed);
  // a failure is one row whose first column is NUL + "fail"
  const failed = rows.$ === CID(Con) && rows.head.$ === CID(Con) && rows.head.head === "\0fail";
  if (!failed) bp_db_keep(key, rows, bp_db_size(rows), bp_db_tables(sql), bp_db_scope(sql, params, typed));
  return rows;
}
function db_query(h, sql, params, limit) {
  return bp_db_read(h, sql, params, limit, false);
}
function db_query_t(h, sql, params, limit) {
  return bp_db_read(h, sql, params, limit, true);
}

// The fixed review-history boundary: no caller SQL, statement cache, retained
// history, or eagerly materialized result set. Metadata lookahead checks the
// byte budget before a payload crosses from SQLite into JavaScript.
// A page is kept as a read is (bp_db_kept): it reads entry rows of one
// thread (thread=?1 AND ... below), so only a write that can change those
// drops it (~10 ms a render on the huge state's review history, 2026-11-05).
function db_review_page(h, thread, after, through, budget) {
  const key = typeof thread === "string" && budget && typeof budget === "object"
    ? JSON.stringify(["review", h, thread, String(after), String(through), budget.rows, budget.bytes]) : null;
  const kept = key === null ? undefined : bp_db_kept(key);
  if (kept !== undefined) return kept;
  const page = bp_db_review_page(h, thread, after, through, budget);
  if (key !== null && page.error === "") bp_db_keep(key, page, bp_db_size(page.rows), new Set(["entry"]), { table: "entry", col: "thread", value: thread });
  return page;
}
function bp_db_review_page(h, thread, after, through, budget) {
  const fail = () => ({ $: CID(HistoryPage), rows: bp_db_from([]), next: after,
    done: false, error: 'indexed review page unavailable' });
  const db = bp_db_get(h);
  let scan, read, result, saved = false;
  try {
    const a = Number(after), z = Number(through);
    if (!db || typeof thread !== 'string' || !thread || thread.includes('\0')
        || !Number.isSafeInteger(a) || a < 0 || !Number.isSafeInteger(z) || z < a
        || !budget || !Number.isInteger(budget.rows) || budget.rows < 1 || budget.rows > 128
        || !Number.isInteger(budget.bytes) || budget.bytes < 256 || budget.bytes > 1048576) throw Error('request');
    const cols = ['seq', 'ord', 'id', 'thread', 'src', 'kind', 'role', 'akind', 'text', 'at'];
    const where = " FROM entry INDEXED BY entry_thread WHERE thread=?1 AND kind='act' AND role='review' AND seq>?2 AND seq<=?3";
    const cost = '256+' + cols.map(c => `coalesce(length(CAST(${c} AS BLOB)),0)`).join('+');
    db.exec('SAVEPOINT bp_review_page'); saved = true;
    scan = db.prepare('SELECT seq,' + cost + ' AS cost' + where + ' ORDER BY seq LIMIT ?4');
    read = db.prepare('SELECT ' + cols.join(',') + where + ' AND seq=?4');
    const out = []; let bytes = 0, next = a, more = false;
    for (const meta of scan.iterate(thread, BigInt(a), BigInt(z), budget.rows + 1)) {
      const seq = Number(meta.seq), charge = Number(meta.cost);
      if (!Number.isSafeInteger(seq) || seq <= next || seq > z || !Number.isSafeInteger(charge) || charge < 256) throw Error('index');
      if (out.length >= budget.rows || (out.length && bytes + charge > budget.bytes)) { more = true; break; }
      const row = read.get(thread, BigInt(a), BigInt(z), BigInt(seq));
      if (!row) throw Error('missing');
      out.push(bp_db_from(cols.map(c => bp_db_text(row[c])))); bytes += charge; next = seq;
    }
    result = { $: CID(HistoryPage), rows: bp_db_from(out), next,
      done: !more, error: '' };
  } catch (_) { result = fail(); }
  finally {
    try { if (scan) scan.finalize(); } catch (_) { result = fail(); }
    finally {
      try { if (read) read.finalize(); } catch (_) { result = fail(); }
      finally {
        try { if (saved) db.exec('RELEASE bp_review_page'); } catch (_) { result = fail(); }
      }
    }
  }
  return result;
}
function db_close(h) {
  bp_db_wrote();
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
io_eff(CID(Db.query), db_query);
io_eff(CID(Db.close), db_close);
io_eff(CID(Db.run.t), db_run_t);
io_eff(CID(Db.batch.t), db_batch_t);
io_eff(CID(Db.script.t), db_script_t);
io_eff(CID(Db.query.t), db_query_t);
io_eff(CID(Db.review_page), db_review_page);
