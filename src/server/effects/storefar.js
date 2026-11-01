// storefar.bend's Fl.lines.read: the complete lines of a file from byte
// `from`, about `budget` bytes of them (always at least one whole line,
// however long) and none that ends past `to`; each as "<bytes> <text>",
// its length in bytes (no newline) then its text decoded as UTF-8, oldest
// first. Empty when there is no complete line before `to`. The bytes stay
// in JavaScript: a Bend list of them took gigabytes on a 7 MB line.
// A line over 256 KB has each string literal over 64 KB emptied, as text:
// nothing else in the line changes (keys, numbers, order). The refill reads
// only ids from a change (far.bend's Fr.fact: a thread, its project, an
// ask's thread, a task's parent), and parsing a megabytes-long text in
// Bend held a gigabyte; the line's byte length is still the original's.
function far_short(text) {
  if (text.length <= 262144) return text;
  let out = '', i = 0, from = 0;
  while (i < text.length) {
    if (text.charCodeAt(i) !== 34) { i++; continue; }
    let j = i + 1;
    while (j < text.length && text.charCodeAt(j) !== 34) j += text.charCodeAt(j) === 92 ? 2 : 1;
    if (j - i - 1 > 65536) { out += text.slice(from, i + 1); from = j; }
    i = j + 1;
  }
  return out + text.slice(from);
}

function far_lines(path, from, to, budget) {
  const fs = require('fs');
  let out = { $: CID(Nil) };
  const a = Number(from), b = Number(to), want = Math.max(1, Number(budget));
  if (!(b > a)) return out;
  const fd = fs.openSync(path, 'r');
  try {
    let size = Math.min(want, b - a), buf, got;
    for (;;) {
      buf = Buffer.allocUnsafe(size);
      got = fs.readSync(fd, buf, 0, size, a);
      if (buf.subarray(0, got).indexOf(10) >= 0 || got < size || size >= b - a) break;
      size = Math.min(size * 2, b - a);
    }
    buf = buf.subarray(0, got);
    const lines = [];
    let start = 0;
    for (;;) {
      const nl = buf.indexOf(10, start);
      if (nl < 0) break;
      lines.push(String(nl - start) + ' ' + far_short(buf.toString('utf8', start, nl)));
      start = nl + 1;
      if (start >= want) break;
    }
    for (let i = lines.length - 1; i >= 0; i--) out = { $: CID(Con), head: lines[i], tail: out };
    return out;
  } finally {
    fs.closeSync(fd);
  }
}
io_eff(CID(Fl.lines.read), far_lines);
