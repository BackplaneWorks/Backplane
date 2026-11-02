'use strict';
const nativeFs = require('node:fs'), path = require('node:path');
const { PREFIX, LIMIT, MAX_BYTES, TTL, sanitize, retained } = require('./connection-record.cjs');
// At most one current file and one replacement, each <= MAX_BYTES/2.
// The durable off marker is one byte. The 128 KiB total cap is checked
// against measured logical AND allocated bytes, including the replacement.
const FILE_BYTES = MAX_BYTES / 2, DISK_CAP_BYTES = 128 * 1024;
function createConnectionLog(home, { now = Date.now, fs = nativeFs } = {}) {
  const file = path.join(home, 'connection-log.json'), temporary = file + '.tmp', control = file + '.off';
  let rows = [], bucket = 0, writes = 0, on = true;
  try { on = !fs.existsSync(control); } catch { on = false; }
  try { fs.unlinkSync(temporary); } catch {}
  try { const s = fs.lstatSync(file);if (s.isFile() && s.nlink === 1) { if (s.size <= FILE_BYTES) rows = retained(JSON.parse(fs.readFileSync(file)), now());else fs.unlinkSync(file); } } catch {}
  function usage() {
    let bytes = 0, allocatedBytes = 0;
    for (const p of [file, temporary, control]) try { const s = fs.lstatSync(p);bytes += s.size;allocatedBytes += Math.max(s.size, (s.blocks || 0) * 512); } catch {}
    return { bytes, allocatedBytes };
  }
  function persist() {
    let text = JSON.stringify(rows) + '\n';
    while (Buffer.byteLength(text) > FILE_BYTES && rows.length) { rows.shift();text = JSON.stringify(rows) + '\n'; }
    try {
      fs.mkdirSync(home, { recursive: true });
      const fd = fs.openSync(temporary, fs.constants.O_WRONLY | fs.constants.O_CREAT | fs.constants.O_TRUNC | (fs.constants.O_NOFOLLOW || 0), 0o600);
      try { fs.fchmodSync(fd, 0o600);fs.writeFileSync(fd, text); } finally { fs.closeSync(fd); }
      const measured = usage();if (Math.max(measured.bytes, measured.allocatedBytes) > DISK_CAP_BYTES) throw Error('Diagnostic storage cap');
      fs.renameSync(temporary, file);return true;
    } catch { try { fs.unlinkSync(temporary); } catch {}return false; }
  }
  function prune() {
    const kept = retained(rows, now());
    if (kept.length !== rows.length) { rows = kept;persist(); }
  }
  prune();
  function record(input) {
    if (!on) return false;
    const value = sanitize({ ...input, at: now(), process: process.pid });
    if (!value) return false;
    const second = Math.floor(now() / 1000);
    if (second !== bucket) { bucket = second; writes = 0; }
    if (++writes > 12) return false;
    rows = retained(rows, now()).concat(value).slice(-LIMIT);
    return persist();
  }
  function capture(message) {
    if (typeof message !== 'string' || message.length > 2048 || !message.startsWith(PREFIX)) return false;
    try { return record(JSON.parse(message.slice(PREFIX.length))); } catch { return false; }
  }
  function snapshot() {
    prune();const { bytes, allocatedBytes } = usage();
    return { enabled: on, bytes, allocatedBytes, capBytes: DISK_CAP_BYTES, retentionDays: TTL / 86400000, records: rows.map(sanitize) };
  }
  function command(op) {
    if (!['on','off','clear'].includes(op)) throw Error('Invalid diagnostics control');
    fs.mkdirSync(home, { recursive: true });
    if (op === 'off') {
      on = false;
      const fd = fs.openSync(control, fs.constants.O_WRONLY | fs.constants.O_CREAT | fs.constants.O_TRUNC | (fs.constants.O_NOFOLLOW || 0), 0o600);
      try { fs.writeFileSync(fd, '0'); } finally { fs.closeSync(fd); }
    } else if (op === 'on') { if (fs.existsSync(control)) fs.unlinkSync(control);on = true; }
    else { rows = [];for (const p of [file, temporary]) if (fs.existsSync(p)) fs.unlinkSync(p); }
    const result = snapshot();if (Math.max(result.bytes, result.allocatedBytes) > DISK_CAP_BYTES) throw Error('Diagnostic storage cap exceeded. Clear local diagnostics.');
    return result;
  }
  return { record, capture, file, snapshot, command };
}
module.exports = { createConnectionLog, FILE_BYTES, DISK_CAP_BYTES };
