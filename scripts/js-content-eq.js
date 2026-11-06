// Server-only exact structural JSON equality. Identity is a sufficient
// equality proof for immutable Bend values; no key/hash is involved.
//
// The view delta (thin-delta.bend) asks this at every node it walks, top
// down, so without memory each level compared again everything the level
// above had already compared up to the first difference: a reply's
// unchanged sidebar was walked once per level of depth, 5-20 ms a reply on
// a real home (2026-11-05). Bend values never change once built, so whether
// two objects or arrays are equal never changes either: each comparison
// remembers its answer for that very pair of objects (weakly, by identity,
// one partner per object) and the next question about the pair is one
// lookup. An unequal answer is remembered for every object/array pair still
// open above the difference. The answer is always the full comparison's.
const __bpEqSame = new WeakMap(), __bpEqDiff = new WeakMap();
// a constructor's tag to its kind, read once per tag (no slice per node)
const __bpEqKind = new Map();
function __bpEqKindOf(tag) {
  let k = __bpEqKind.get(tag);
  if (k === undefined) {
    k = ['Null', 'End', 'Flag', 'Num', 'Str', 'Arr', 'Obj', 'Item', 'Field'].indexOf(tag.slice(tag.lastIndexOf('.') + 1));
    __bpEqKind.set(tag, k);
  }
  return k;
}
function __bpContentEq(a, b, fallback) {
  // the pairs still to compare as three parallel stacks (no array per pair);
  // an exit frame (ex 1) marks an object/array pair whose children all
  // compared equal. Those still on the stack at a difference are its
  // ancestors, so they are unequal too.
  const xs = [a], ys = [b], ex = [0];
  const differ = () => {
    for (let i = 0; i < xs.length; i++) if (ex[i] === 1) __bpEqDiff.set(xs[i], ys[i]);
    return false;
  };
  while (xs.length) {
    const a = xs.pop(), b = ys.pop(), exit = ex.pop();
    if (exit === 1) { __bpEqSame.set(a, b); continue; }
    if (a === b) continue;
    const tag = a.$;
    if (tag !== b.$) return differ();
    switch (__bpEqKindOf(tag)) {
      case 0: case 1: break;
      case 2: if (a.value !== b.value) return differ(); break;
      case 3: if (a.raw !== b.raw) return differ(); break;
      case 4: if (a.text !== b.text) return differ(); break;
      case 5:
        if (__bpEqSame.get(a) === b) break;
        if (__bpEqDiff.get(a) === b) return differ();
        xs.push(a, a.items); ys.push(b, b.items); ex.push(1, 0); break;
      case 6:
        if (__bpEqSame.get(a) === b) break;
        if (__bpEqDiff.get(a) === b) return differ();
        xs.push(a, a.fields); ys.push(b, b.fields); ex.push(1, 0); break;
      case 7: xs.push(a.tail, a.head); ys.push(b.tail, b.head); ex.push(0, 0); break;
      case 8:
        if (a.key !== b.key) return differ();
        xs.push(a.tail, a.value); ys.push(b.tail, b.value); ex.push(0, 0); break;
      default: if (!fallback(a, b)) return differ();
    }
  }
  return true;
}
