// hotboot.bend's effects: the log's lines in a byte range, read in one
// pass outside Bend (a Bend list of every byte of a 200 MB log is gigabytes
// in JavaScript). Lines are the text between newlines, decoded as UTF-8,
// newest first. Left out: empty lines, and every line that starts inside
// one of the skipped ranges (skips: [a1, b1, a2, b2, ...] as decimal text,
// a <= start < b): the stretches the store gave no number (a torn fragment).
// Each line is as clients see it (wire below).
// A turn's input setting goes out with its value blank, as codec.bend's
// Change.wire_line writes it (model.bend's Setting.input): the lines were
// logged before the store kept a wire text for them. Only the key and the
// time are decoded, from the bytes: a slice of the whole line's string
// would keep all of it alive.
const INPUTS = ['{"$":"SettingSet","key":"launch.input.', '{"$":"SettingSet","key":"fallback.input.'].map((t) => Buffer.from(t));
const VALUE = Buffer.from('","value":"');
const AT = Buffer.from(',"at":');
function input(buf, start, end) {
  for (const p of INPUTS) if (end - start > p.length && buf.compare(p, 0, p.length, start, start + p.length) === 0) return true;
  return false;
}
function wire(buf, start, end) {
  if (!input(buf, start, end)) return buf.toString("utf8", start, end);
  const v = buf.indexOf(VALUE, start);
  const at = buf.lastIndexOf(AT, end - AT.length);
  if (v < 0 || v >= end || at < v || buf[end - 1] !== 125) return buf.toString("utf8", start, end);
  return buf.toString("utf8", start, v) + '","value":"","at":' + buf.toString("utf8", at + AT.length, end - 1) + "}";
}

function hot_lines(path, from, to, skips) {
  let out = { $: CID(Nil) };
  try {
    const sk = [];
    for (let xs = skips; xs.$ === CID(Con); xs = xs.tail) sk.push(Number(xs.head));
    const skipped = (s) => {
      for (let i = 0; i + 1 < sk.length; i += 2) if (s >= sk[i] && s < sk[i + 1]) return true;
      return false;
    };
    const fs = require("fs");
    const a = Number(from);
    const b = Number(to);
    if (!(b > a)) return out;
    const fd = fs.openSync(path, "r");
    let buf;
    try {
      buf = Buffer.allocUnsafe(b - a);
      let got = 0;
      while (got < b - a) {
        const n = fs.readSync(fd, buf, got, b - a - got, a + got);
        if (n <= 0) break;
        got += n;
      }
      buf = buf.subarray(0, got);
    } finally {
      fs.closeSync(fd);
    }
    let start = 0;
    for (;;) {
      const nl = buf.indexOf(10, start);
      const end = nl < 0 ? buf.length : nl;
      if (end > start && !(sk.length && skipped(a + start))) out = { $: CID(Con), head: wire(buf, start, end), tail: out };
      if (nl < 0) break;
      start = nl + 1;
    }
    return out;
  } catch (e) {
    return { $: CID(Nil) };
  }
}
io_eff(CID(Hot.lines), hot_lines);
