// EvLog.read (host.bend), as evlog.c: the tail's fix, then the event log's whole lines as
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

function evlog_read(path) {
  let b;
  try {
    b = require("fs").readFileSync(path);
  } catch (_) {
    return { $: "Con", head: "", tail: { $: "Nil" } };
  }
  const n = b.length;
  if (n === 0) return { $: "Con", head: "", tail: { $: "Nil" } };
  const td = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
  let end = n;
  while (end > 0 && b[end - 1] !== 10) end -= 1;
  let xs = { $: "Nil" };
  while (end > 0) {
    const stop = end - 1;
    const start = stop > 0 ? b.lastIndexOf(10, stop - 1) + 1 : 0;
    xs = { $: "Con", head: evlog_line(td, b, start, stop), tail: xs };
    end = start;
  }
  return { $: "Con", head: b[n - 1] === 10 ? "" : "\n", tail: xs };
}

io_eff(CID(EvLog.read), evlog_read);
