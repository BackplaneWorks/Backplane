// Streaming tar state machine; no filesystem, subprocess or fixture IO.
// Python tarfile's default PAX float-mtime headers are ignored as metadata.
const P = require('./real_store_active_policy.cjs');
const check = (v, code) => { if (!v) throw Error(code); };
class TarStream {
  constructor(sink) { this.sink = sink; this.seen = new Set(); this.header = Buffer.alloc(0);
    this.left = 0; this.padding = 0; this.zeros = 0; this.expanded = 0; this.total = 0;
    this.pax = null; this.pendingPax = false; this.member = null; this.paxHeaders = 0; }
  octal(b) { const s = b.toString('ascii'); check(/^[ 0-7]+\0?[ ]*$/.test(s), 'archive-number'); return parseInt(s.replace(/\0/g, '').trim(), 8); }
  paxMetadata(b) {
    let at = 0; const keys = new Set();
    while (at < b.length) {
      const space = b.indexOf(32, at); check(space > at && space - at <= 6, 'archive-pax-record');
      const lengthText = b.subarray(at, space).toString('ascii'); check(/^[1-9][0-9]*$/.test(lengthText), 'archive-pax-record');
      const n = Number(lengthText), end = at + n; check(end <= b.length && end > space + 2 && b[end - 1] === 10, 'archive-pax-record');
      const text = b.subarray(space + 1, end - 1).toString('utf8'), eq = text.indexOf('='); check(eq > 0, 'archive-pax-record');
      const key = text.slice(0, eq), value = text.slice(eq + 1);
      check(!keys.has(key) && ['mtime', 'atime', 'ctime', 'uid', 'gid', 'uname', 'gname'].includes(key), 'archive-pax-override');
      keys.add(key); check(keys.size <= 7 && value.length <= 64 && !/[\0\r\n]/.test(value), 'archive-pax-value');
      if (['mtime', 'atime', 'ctime'].includes(key)) check(/^-?[0-9]+(?:\.[0-9]+)?$/.test(value) && Number.isFinite(Number(value)), 'archive-pax-value');
      if (['uid', 'gid'].includes(key)) check(/^[0-9]+$/.test(value), 'archive-pax-value');
      at = end;
    }
    check(at === b.length && keys.size > 0, 'archive-pax-record');
  }
  push(chunk) {
    let b = Buffer.from(chunk); this.expanded += b.length; check(this.expanded <= 8 * 1024 ** 3 + 1048576, 'archive-expanded-bound');
    while (b.length) {
      if (this.left) {
        const n = Math.min(this.left, b.length), data = b.subarray(0, n);
        if (this.pax !== null) this.pax = Buffer.concat([this.pax, data]); else this.sink.data(data);
        this.left -= n; b = b.subarray(n);
        if (!this.left) {
          if (this.pax !== null) { this.paxMetadata(this.pax); this.pax = null; this.pendingPax = true; }
          else { this.sink.end(this.member); this.member = null; }
        }
        continue;
      }
      if (this.padding) {
        const n = Math.min(this.padding, b.length); check(b.subarray(0, n).every(x => x === 0), 'archive-padding');
        this.padding -= n; b = b.subarray(n); continue;
      }
      const n = Math.min(512 - this.header.length, b.length);
      this.header = Buffer.concat([this.header, b.subarray(0, n)]); b = b.subarray(n); if (this.header.length < 512) continue;
      const h = this.header; this.header = Buffer.alloc(0);
      if (h.every(x => x === 0)) { check(!this.pendingPax, 'archive-dangling-pax'); this.zeros++; continue; }
      check(this.zeros === 0, 'archive-trailing-data');
      const field = (a, z) => h.subarray(a, z).toString('utf8').replace(/\0.*$/s, '');
      check(this.octal(h.subarray(148, 156)) === [...h].reduce((s, v, i) => s + (i >= 148 && i < 156 ? 32 : v), 0), 'archive-checksum');
      check(field(345, 500) === '', 'archive-prefix');
      const name = field(0, 100), size = this.octal(h.subarray(124, 136)), type = String.fromCharCode(h[156]);
      if (type === 'x') {
        check(!this.pendingPax && size > 0 && size <= 16384 && ++this.paxHeaders <= 4, 'archive-pax-bound'); this.pax = Buffer.alloc(0);
      } else {
        P.archiveMember(name, type, size, this.seen); this.pendingPax = false; this.member = name;
        this.total += size; check(this.total <= 8 * 1024 ** 3, 'archive-total'); this.sink.begin(name, size);
      }
      this.left = size; this.padding = (512 - size % 512) % 512;
    }
  }
  finish() {
    check(this.left === 0 && this.padding === 0 && this.header.length === 0 && this.zeros >= 2 &&
      !this.pendingPax && this.pax === null && ['RECEIPT.json', 'events.jsonl', 'store.db'].every(n => this.seen.has(n)), 'archive-incomplete');
    return { members: this.seen.size, extractedBytes: this.total, paxHeaders: this.paxHeaders };
  }
}
module.exports = { TarStream };
