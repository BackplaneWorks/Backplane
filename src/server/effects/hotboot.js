// hotboot.bend's effects: the log's lines in a byte range, read in one
// pass outside Bend (a Bend list of every byte of a 200 MB log is gigabytes
// in JavaScript). Lines are the text between newlines, decoded as UTF-8,
// newest first; empty lines are left out (they never parse).
function hot_lines(path, from, to) {
  let out = { $: CID(Nil) };
  try {
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
      if (end > start) out = { $: CID(Con), head: buf.toString("utf8", start, end), tail: out };
      if (nl < 0) break;
      start = nl + 1;
    }
    return out;
  } catch (e) {
    return { $: CID(Nil) };
  }
}
io_eff(CID(Hot.lines), hot_lines);
