// Server-only implementations of Base Map.bit and Map.diff. No cache: keys,
// values and ownership are never retained. js-native.py fences the complete
// emitted definitions/callees before replacing either entry point.
function __bpMapBitValue(key, pos) {
  const ci = Math.floor(pos / 33), off = pos - ci * 33;
  // Without high surrogates each UTF-16 unit is exactly one Bend character.
  // Native scanning avoids walking a thread/settings prefix at every trie
  // edge. A high surrogate (paired or lone) always takes the exact slow path.
  if (!/[\ud800-\udbff]/.test(key)) {
    return ci < key.length && (off === 0 || ((key.charCodeAt(ci) >>> (32 - off)) & 1) !== 0);
  }
  let i = 0, n = 0;
  while (i < key.length) {
    const cp = key.codePointAt(i);
    if (n === ci) return off === 0 || ((cp >>> (32 - off)) & 1) !== 0;
    i += cp > 0xffff ? 2 : 1;
    n++;
  }
  return false;
}

function __bpMapBit(key, pos) {
  return {$: "Tuple", fst: key, snd: __bpMapBitValue(key, pos)};
}

function __bpMapDiff(a, b, checked) {
  let i = 0, j = 0, same = 0;
  while (i < a.length && j < b.length) {
    const x = a.codePointAt(i), y = b.codePointAt(j);
    if (x !== y) return checked(same * 33 + 1 + Math.clz32(x ^ y));
    i += x > 0xffff ? 2 : 1;
    j += y > 0xffff ? 2 : 1;
    same++;
  }
  // Base returns the first absent character's presence bit, including equal
  // strings. Lone surrogates are individual code points, just as in Bend JS.
  return checked(same * 33);
}

// Base get/has reconstruct an equal trie along the read path to return the
// linear input. JS values are immutable: retain the original root instead.
// Value identity, missing defaults and leaf keys are preserved; no key is
// cached and no node/value is mutated (including shared older map versions).
function __bpMapRead(map, key, missing, has) {
  let node = map;
  while (node.$ === "MNode") node = __bpMapBitValue(key, node.pos) ? node.hi : node.lo;
  const found = node.$ === "MLeaf" && node.key === key;
  return {$: "Tuple", fst: map, snd: has ? found : found ? node.val : missing};
}
