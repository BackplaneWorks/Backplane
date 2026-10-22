// storedb.bend's Sto.fx.lines: the event log's whole lines from byte off,
// within about max bytes (the first whole line even when it is longer;
// none when no line ends before the file does), each as its byte length
// (decimal) and its text: [len, text, len, text, ...]. Decoded as
// evlog.js decodes (exactly core/utf8.bend's Utf8.decode), from one
// Buffer, never a List cell per byte.

function sto_decode(b, s, e) {
  const out = [];
  let cp = 0, owe = 0;
  for (let i = s; i < e; i += 1) {
    const x = b[i];
    if (owe > 0) {
      if ((x & 192) === 128) {
        cp = (cp << 6) | (x & 63);
        if (--owe === 0) {
          out.push(cp > 1114111 || (cp >= 55296 && cp <= 57343) ? 65533 : cp);
        }
        continue;
      }
      out.push(65533);
      owe = 0;
    }
    if (x < 128) out.push(x);
    else if ((x & 224) === 192) { cp = x & 31; owe = 1; }
    else if ((x & 240) === 224) { cp = x & 15; owe = 2; }
    else if ((x & 248) === 240) { cp = x & 7; owe = 3; }
    else out.push(65533);
  }
  if (owe > 0) out.push(65533);
  let t = "";
  for (let i = 0; i < out.length; i += 4096) t += String.fromCodePoint(...out.slice(i, i + 4096));
  return t;
}

function sto_line(td, b, s, e) {
  let ascii = true;
  for (let i = s; i < e; i += 1) if (b[i] >= 128) { ascii = false; break; }
  if (ascii) return b.latin1Slice ? b.latin1Slice(s, e) : b.toString("latin1", s, e);
  try {
    return td.decode(b.subarray(s, e));
  } catch (_) {
    return sto_decode(b, s, e);
  }
}

function sto_fx_lines(path, off, max) {
  const fs = require("fs");
  const nil = { $: CID(Nil) };
  let fd;
  try { fd = fs.openSync(path, "r"); } catch (_) { return nil; }
  try {
    const size = fs.fstatSync(fd).size;
    const o = Number(off);
    if (o >= size) return nil;
    let want = Math.max(1, Math.min(Number(max), size - o));
    let b, n, last;
    for (;;) {
      b = Buffer.allocUnsafe(want);
      n = fs.readSync(fd, b, 0, want, o);
      last = b.subarray(0, n).lastIndexOf(10);
      if (last >= 0 || o + want >= size) break;
      want = Math.min(want * 2, size - o);
    }
    if (last < 0) return nil;
    const td = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
    const parts = [];
    let s = 0;
    while (s <= last) {
      const e = b.indexOf(10, s);
      parts.push(String(e - s), sto_line(td, b, s, e));
      s = e + 1;
    }
    let xs = nil;
    for (let i = parts.length - 1; i >= 0; i--) xs = { $: CID(Con), head: parts[i], tail: xs };
    return xs;
  } catch (_) {
    return nil;
  } finally {
    fs.closeSync(fd);
  }
}

io_eff(CID(Sto.fx.lines), sto_fx_lines);
