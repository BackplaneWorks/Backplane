// EvLog.read_from (host.bend), as evlog.c: the tail's fix, the end offset, then the event log's whole lines as
// Strings from one Buffer, never a List cell per byte. A line that is
// well-formed UTF-8 goes through TextDecoder (fatal, so anything it accepts
// decodes alike in core/utf8.bend's Utf8.decode); any other line is decoded
// byte by byte exactly as Utf8.decode does.

function evlog_decode(b, s, e) {
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

function evlog_line(td, b, s, e) {
  let ascii = true;
  for (let i = s; i < e; i += 1) if (b[i] >= 128) { ascii = false; break; }
  if (ascii) return b.latin1Slice ? b.latin1Slice(s, e) : b.toString("latin1", s, e);
  try {
    return td.decode(b.subarray(s, e));
  } catch (_) {
    return evlog_decode(b, s, e);
  }
}

// [tail's fix, offset past the last whole line, lines...] from byte off
function evlog_read_from(path, offs) {
  const fs = require("fs");
  const off = Number(/^[0-9]*/.exec(offs)[0] || "0");
  const none = (end) => ({ $: "Con", head: "", tail: { $: "Con", head: String(end), tail: { $: "Nil" } } });
  let b;
  try {
    const fd = fs.openSync(path, "r");
    try {
      const size = fs.fstatSync(fd).size;
      b = Buffer.alloc(Math.max(size - off, 0));
      let got = 0;
      while (got < b.length) {
        const r = fs.readSync(fd, b, got, b.length - got, off + got);
        if (r <= 0) break;
        got += r;
      }
      b = b.subarray(0, got);
    } finally {
      fs.closeSync(fd);
    }
  } catch (_) {
    return none(off);
  }
  const n = b.length;
  if (n === 0) return none(off);
  const td = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
  let end = n;
  while (end > 0 && b[end - 1] !== 10) end -= 1;
  const whole = end;
  let xs = { $: "Nil" };
  while (end > 0) {
    const stop = end - 1;
    const start = stop > 0 ? b.lastIndexOf(10, stop - 1) + 1 : 0;
    xs = { $: "Con", head: evlog_line(td, b, start, stop), tail: xs };
    end = start;
  }
  return { $: "Con", head: b[n - 1] === 10 ? "" : "\n", tail: { $: "Con", head: String(off + whole), tail: xs } };
}

io_eff(CID(EvLog.read_from), evlog_read_from);

// File.read_at's descriptor/offset/max contract, with complete lines and
// their raw byte counts packed directly. The builder retains only these
// strings, never a linked list for every byte of every read. No fadvise,
// cache dropping, or collection; the parser/fold still runs in Bend.
function evlog_read_chunk(file, off, max) {
  try {
    const len = Math.min(max, 2147483647);
    const b = Buffer.alloc(Math.max(len, 1));
    const n = require("fs").readSync(file, b, 0, len, off);
    const td = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
    let end = n;
    while (end > 0 && b[end - 1] !== 10) end--;
    const used = end;
    let xs = { $: "Nil" };
    while (end > 0) {
      const stop = end - 1;
      const start = stop > 0 ? b.lastIndexOf(10, stop - 1) + 1 : 0;
      xs = { $: "Con", head: String(stop - start), tail:
        { $: "Con", head: evlog_line(td, b, start, stop), tail: xs } };
      end = start;
    }
    return io_tup(file, io_done({ $: "Con", head: String(used), tail:
      { $: "Con", head: String(n), tail: xs } }));
  } catch (e) {
    return io_tup(file, io_fail(Math.abs(e.errno ?? 5)));
  }
}

io_eff(CID(EvLog.read_chunk), evlog_read_chunk);
