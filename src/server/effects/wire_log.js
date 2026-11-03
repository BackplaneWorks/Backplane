// No in-memory event history. One indexed cursor page, with a fixed row and
// byte budget; statement and log descriptor are released on every path.
const WIRE_PAGE_ROWS = 1024;
const WIRE_PAGE_BYTES = 1048576;
function wire_log_page(handle, home, since, through) {
  const fs = require('fs'), path = require('path');
  let fd, stmt, db;
  try {
    const a = Number(since), z = Number(through);
    if (!handle || !Number.isSafeInteger(a) || a < 0 || !Number.isSafeInteger(z) || z < a) throw Error('range');
    // Effects execute in separate lexical closures. Never borrow db.js's
    // private slot table. This connection belongs to one bounded page only.
    const { Database } = require('bun:sqlite');
    db = new Database(path.join(home, 'store.db'), { readonly: true, create: false, safeIntegers: true });
    db.exec('PRAGMA busy_timeout=100');
    const values = []; let bytes = 0, next = a + 1, bounded = false;
    stmt = db.prepare('SELECT seq, off, len, wire FROM line WHERE seq > ? AND seq <= ? ORDER BY seq LIMIT 1024');
    for (const row of stmt.iterate(BigInt(a), BigInt(z))) {
      if (Number(row.seq) !== next) throw Error('gap');
      let text = String(row.wire || '');
      if (!text) {
        const off = Number(row.off), len = Number(row.len);
        if (!Number.isSafeInteger(off) || off < 0 || !Number.isSafeInteger(len) || len < 1) throw Error('offset');
        if (values.length && bytes + len > WIRE_PAGE_BYTES) { bounded = true; break; }
        if (fd === undefined) fd = fs.openSync(path.join(home, 'events.jsonl'), fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW || 0));
        const b = Buffer.allocUnsafe(len); let got = 0;
        while (got < len) { const n = fs.readSync(fd, b, got, len - got, off + got); if (n < 1) throw Error('truncated'); got += n; }
        text = b.toString('utf8');
      }
      const cost = Buffer.byteLength(text);
      if (values.length && bytes + cost > WIRE_PAGE_BYTES) { bounded = true; break; }
      values.push(text); bytes += cost; next++;
      // An indivisible oversized event travels alone, never cached or lost.
      if (bytes >= WIRE_PAGE_BYTES || values.length >= WIRE_PAGE_ROWS) { bounded = true; break; }
    }
    if ((!values.length && a < z) || (!bounded && next <= z)) throw Error('missing');
    let lines = {$: CID(Nil)};
    for (let i = values.length - 1; i >= 0; i--) lines = {$: CID(Con), head: values[i], tail: lines};
    return {$: CID(WirePage), lines, error: ''};
  } catch (_) { return {$: CID(WirePage), lines: {$: CID(Nil)}, error: 'indexed wire page unavailable'}; }
  finally {
    try { if (fd !== undefined) fs.closeSync(fd); }
    finally { try { if (stmt) stmt.finalize(); } finally { if (db) db.close(); } }
  }
}
io_eff(CID(WireLog.page), wire_log_page);
