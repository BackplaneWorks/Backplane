// Streaming log validation and the compatibility line reader. A normal hot
// boot retains only its first public wire line plus the count, never the log.
// Read chunks are fixed at 64 KiB. Only a selected line crossing a chunk
// boundary is carried; counting later lines needs no payload allocation.
const INPUTS = ['{"$":"SettingSet","key":"launch.input.', '{"$":"SettingSet","key":"fallback.input.', '{"$":"SettingSet","key":"ride.'].map(t => Buffer.from(t));
const VALUE = Buffer.from('\",\"value\":\"');
const AT = Buffer.from(',"at":');
function input(buf, start, end) {
  for (const p of INPUTS) if (end - start > p.length && buf.compare(p, 0, p.length, start, start + p.length) === 0) return true;
  return false;
}
function wire(buf, start, end) {
  if (!input(buf, start, end)) return buf.toString('utf8', start, end);
  const v = buf.indexOf(VALUE, start), at = buf.lastIndexOf(AT, end - AT.length);
  if (v < 0 || v >= end || at < v || buf[end - 1] !== 125) return buf.toString('utf8', start, end);
  return buf.toString('utf8', start, v) + '\",\"value\":\"\",\"at\":' + buf.toString('utf8', at + AT.length, end - 1) + '}';
}
function hot_walk(path, from, to, skips, all) {
  const fs = require('fs'), a = Number(from), b = Number(to), sk = [];
  for (let xs = skips; xs.$ === CID(Con); xs = xs.tail) sk.push(Number(xs.head));
  const skipped = s => { for (let i = 0; i + 1 < sk.length; i += 2) if (s >= sk[i] && s < sk[i + 1]) return true; return false; };
  let lines = {$: CID(Nil)}, first = '', count = 0;
  if (!(b > a)) return {lines, first, count};
  if (!Number.isSafeInteger(a) || a < 0 || !Number.isSafeInteger(b)) throw Error('Invalid log range');
  const fd = fs.openSync(path, 'r'), buf = Buffer.allocUnsafe(65536);
  let pos = a, lineAt = a, pieces = [], pieceBytes = 0;
  const finish = end => {
    if (end > lineAt && !skipped(lineAt)) {
      if (all || count === 0) {
        const line = pieces.length === 1 ? pieces[0] : Buffer.concat(pieces, pieceBytes);
        const text = wire(line, 0, line.length);
        if (count === 0) first = text;
        if (all) lines = {$: CID(Con), head: text, tail: lines};
      }
      count++;
    }
    pieces = []; pieceBytes = 0; lineAt = end + 1;
  };
  try {
    while (pos < b) {
      const n = fs.readSync(fd, buf, 0, Math.min(buf.length, b - pos), pos);
      if (!n) throw Error('Truncated log range');
      let start = 0;
      while (start < n) {
        const nl = buf.indexOf(10, start), end = nl >= 0 && nl < n ? nl : n;
        if ((all || count === 0) && !skipped(lineAt) && end > start) {
          const part = Buffer.from(buf.subarray(start, end)); pieces.push(part); pieceBytes += part.length;
        }
        if (end < n) { finish(pos + end); start = end + 1; }
        else break;
      }
      pos += n;
    }
    if (lineAt < b) finish(b);
    return {lines, first, count};
  } finally { fs.closeSync(fd); }
}
function hot_lines(path, from, to, skips) {
  try { return hot_walk(path, from, to, skips, true).lines; }
  catch (_) { return {$: CID(Nil)}; }
}
function hot_scan(path, from, to, skips) {
  try { const r = hot_walk(path, from, to, skips, false); return {$: CID(WireScan), count: r.count, first: r.first, error: ''}; }
  catch (_) { return {$: CID(WireScan), count: 0, first: '', error: 'log range could not be scanned'}; }
}
io_eff(CID(Hot.lines), hot_lines);
io_eff(CID(Hot.scan), hot_scan);
