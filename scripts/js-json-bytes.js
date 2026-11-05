// Server-only core/json.bend byte counts. No IO; the one cache is
// __bpJsonBytes' sizes by identity below.
// for..of follows the reference's code-point slicing, including lone UTF-16
// surrogates. Count their three bytes directly; TextEncoder replaces them.
function __bpJsonBytesEsc(s, n, check) {
  for (const ch of s) {
    const c = ch.codePointAt(0);
    n = check(n + (c < 32 ? (c === 10 || c === 13 || c === 9 ? 2 : 6) :
      c === 34 || c === 92 ? 2 : c < 128 ? 1 : c < 2048 ? 2 : c < 65536 ? 3 : 4));
  }
  return n;
}

function __bpJsonBytesStr(s, n, check) {
  for (const ch of s) {
    const c = ch.codePointAt(0);
    n = check(n + (c < 128 ? 1 : c < 2048 ? 2 : c < 65536 ? 3 : 4));
  }
  return n;
}

// Server-only Json.bytes(j): the size in UTF-8 bytes of Json.show(j), as
// Json.bytes.go(j, true, 0) adds it up, without re-walking what it measured
// before. A reply is measured three or four times on its way out (both
// fitting checks, respond, deliver), each time a fresh envelope around the
// same tree, and the hub measured all of it each time (2026-11-05: 3-7 ms a
// view on a real home). Bend values never change once built, so an object's
// or array's size never changes either: each is remembered by identity
// (weakly) and counted once. Every other constructor is counted exactly as
// Json.bytes.go counts it, malformed chains included; anything it does not
// know goes to the pure count. The running total only grows, so checking
// the Nat bound once at the end throws exactly when the pure count would.
const __bpJsonSize = new WeakMap();
// a constructor's tag to its kind, read once per tag (no slice per node)
const __bpJsonKind = new Map();
function __bpJsonKindOf(tag) {
  let k = __bpJsonKind.get(tag);
  if (k === undefined) {
    k = ['Null', 'Flag', 'Num', 'Str', 'Arr', 'Obj', 'Item', 'Field', 'End'].indexOf(tag.slice(tag.lastIndexOf('.') + 1));
    __bpJsonKind.set(tag, k);
  }
  return k;
}
function __bpJsonBytes(j, fallback, check) {
  let n = 0;
  // what is still to count, as parallel stacks: a node and whether it is
  // first in its chain, or (an exit, 2) an object/array to remember with
  // the total it started at
  const xs = [j], fs = [1], at = [0];
  while (xs.length) {
    const x = xs.pop(), f = fs.pop(), start = at.pop();
    if (f === 2) { __bpJsonSize.set(x, n - start); continue; }
    switch (__bpJsonKindOf(x.$)) {
      case 0: n += 4; break;
      case 1: n += x.value ? 4 : 5; break;
      case 2: n = __bpJsonBytesStr(x.raw, n, check); break;
      case 3: n = __bpJsonBytesEsc(x.text, n + 2, check); break;
      case 4: case 5: {
        const known = __bpJsonSize.get(x);
        if (known !== undefined) { n += known; break; }
        xs.push(x, x.items !== undefined ? x.items : x.fields); fs.push(2, 1); at.push(n, 0);
        n += 2;
        break;
      }
      case 6:
        if (f === 0) n += 1;
        xs.push(x.tail, x.head); fs.push(0, 1); at.push(0, 0);
        break;
      case 7:
        if (f === 0) n += 1;
        n = __bpJsonBytesEsc(x.key, n + 2, check) + 1;
        xs.push(x.tail, x.value); fs.push(0, 1); at.push(0, 0);
        break;
      case 8: break;
      default: n += fallback(x, f === 1, 0);
    }
  }
  return check(n);
}
