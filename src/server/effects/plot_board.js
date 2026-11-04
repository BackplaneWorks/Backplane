// A board's top-level forms for its --plot child (src/server/plot.bend's
// PlotFx.board). Read as a Bend byte list, an 85 MB board took 4 GB of list
// cells and its forms as many again, before parsing began: the child ran
// out of memory at 12 GB. Here the file is cut in one pass over its bytes,
// the same cut as core/sexpr.bend's Sx.forms, and each form is kept as
// latin1 text (one char per byte, so its bytes come back exactly), parsed
// one by one. Answers None for an unfinished file, Some([]) for an empty or
// unreadable one, else Some(head <> forms) with head the first 8192 bytes.
function plot_board(path) {
  let b;
  try { b = require('node:fs').readFileSync(path); } catch { return {$: CID(Some), value: {$: CID(Nil)}}; }
  if (!b.length) return {$: CID(Some), value: {$: CID(Nil)}};
  // depth (a U32, as Cut's), inside a string, after a backslash, where the
  // form being read starts
  let d = 0, s = false, e = false, start = 0;
  const forms = [];
  for (let i = 0; i < b.length; i++) {
    const c = b[i];
    if (s) {
      if (e) e = false;
      else if (c === 92) e = true;
      else if (c === 34) s = false;
    } else if (c === 34) {
      s = true;
    } else if (c === 40) {
      if (d === 1) start = i;
      d = (d + 1) >>> 0;
    } else if (c === 41) {
      if (d === 2) forms.push(b.latin1Slice(start, i + 1));
      d = (d - 1) >>> 0;
    }
  }
  if (d !== 0 || s) return {$: CID(None)};
  let xs = {$: CID(Nil)};
  for (let i = forms.length - 1; i >= 0; i--) xs = {$: CID(Con), head: forms[i], tail: xs};
  return {$: CID(Some), value: {$: CID(Con), head: b.latin1Slice(0, Math.min(b.length, 8192)), tail: xs}};
}

io_eff(CID(PlotFx.board), plot_board);
