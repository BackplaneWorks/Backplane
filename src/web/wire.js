// The hub's binary frames (core/cbor.bend's wire format) decoded in plain
// JavaScript, straight into Bend's Json tree ({$: T.Obj, fields: Field
// chain}, ...), as Cbor.decode reads them. Bend's own decoder walks a list
// cell per byte and was most of a page load on a long log; the phones'
// bridge and the web page both use this one. keys and words are the
// dictionaries (Cbor.keys(), Cbor.words()), read from Bend so they never drift.

// Bend 2.0.36 tags a module type's constructors with their file ("../core/json.Str",
// "dom.El": the path depends on the bundle's entry), so JavaScript never spells them:
// learn() reads them off the witness values app.bend's json_tags()/dom_tags() build,
// and is()/kind() read a tag by its name whatever the path.
export const T = { Arr: "Arr", Item: "Item", End: "End", Null: "Null", Flag: "Flag", Num: "Num", Str: "Str", Obj: "Obj", Field: "Field", El: "El", Txt: "Txt", At: "At" };
export const kind = (x) => {
  const t = x.$;
  const i = t.lastIndexOf(".");
  return i < 0 ? t : t.slice(i + 1);
};
export const is = (x, name) => kind(x) === name;
export function learnJson(w) {
  const i1 = w.items, i2 = i1.tail, i3 = i2.tail, i4 = i3.tail, i5 = i4.tail;
  T.Arr = w.$; T.Item = i1.$; T.Null = i1.head.$; T.Flag = i2.head.$; T.Num = i3.head.$;
  T.Str = i4.head.$; T.Obj = i5.head.$; T.Field = i5.head.fields.$; T.End = i5.head.fields.tail.$;
}
export function learnDom(n) {
  T.El = n.$; T.At = n.attrs.head.$; T.Txt = n.kids.head.$;
}

// UTF-8 bytes [i, end) as a string
export function utf8(b, i, end) {
  let s = "";
  const cs = [];
  while (i < end) {
    let c = b[i++];
    if (c >= 0xf0) c = ((c & 7) << 18) | ((b[i++] & 63) << 12) | ((b[i++] & 63) << 6) | (b[i++] & 63);
    else if (c >= 0xe0) c = ((c & 15) << 12) | ((b[i++] & 63) << 6) | (b[i++] & 63);
    else if (c >= 0xc0) c = ((c & 31) << 6) | (b[i++] & 63);
    if (c > 0xffff) {
      c -= 0x10000;
      cs.push(0xd800 | (c >> 10), 0xdc00 | (c & 1023));
    } else cs.push(c);
    if (cs.length > 4096) {
      s += String.fromCharCode.apply(null, cs);
      cs.length = 0;
    }
  }
  return s + String.fromCharCode.apply(null, cs);
}

// CBOR bytes to Bend's Json tree, as Cbor.decode reads them; malformed or
// truncated bytes are Null, as there
export function cbor(b, KEYS, WORDS) {
  let i = 0;
  const bad = () => {
    throw new Error("cbor");
  };
  const arg = (ai) => {
    if (ai < 24) return ai;
    if (ai === 24) return b[i++];
    if (ai === 25) return (b[i++] << 8) | b[i++];
    if (ai === 26) return ((b[i++] << 24) | (b[i++] << 16) | (b[i++] << 8) | b[i++]) >>> 0;
    return bad();
  };
  const text = (n) => {
    if (i + n > b.length) bad();
    const s = utf8(b, i, i + n);
    i += n;
    return s;
  };
  // tag: 0 none, 6 a dictionary word, 7 a number's raw text
  const item = (tag) => {
    if (i >= b.length) bad();
    const h = b[i++], maj = h >> 5, ai = h & 31;
    if (maj === 6) return tag === 0 && (ai === 6 || ai === 7) ? item(ai) : bad();
    if (maj === 7) {
      if (tag !== 0) bad();
      if (ai === 20) return { $: T.Flag, value: false };
      if (ai === 21) return { $: T.Flag, value: true };
      if (ai === 22) return { $: T.Null };
      return bad();
    }
    const n = arg(ai);
    if (i > b.length) bad();
    if (maj === 0 && tag === 0) return { $: T.Num, raw: String(n) };
    if (maj === 0 && tag === 6) return n < WORDS.length ? { $: T.Str, text: WORDS[n] } : bad();
    if (maj === 1 && tag === 0) return { $: T.Num, raw: String(-1 - n) };
    if (maj === 3 && tag === 0) return { $: T.Str, text: text(n) };
    if (maj === 3 && tag === 7) return { $: T.Num, raw: text(n) };
    if (maj === 4 && tag === 0) {
      const vs = [];
      for (let k = 0; k < n; k += 1) vs.push(item(0));
      let items = { $: T.End };
      for (let k = n - 1; k >= 0; k -= 1) items = { $: T.Item, head: vs[k], tail: items };
      return { $: T.Arr, items };
    }
    if (maj === 5 && tag === 0) {
      const ks = [], vs = [];
      for (let k = 0; k < n; k += 1) {
        const key = item(0);
        if (is(key, "Str")) ks.push(key.text);
        else if (is(key, "Num") && /^[0-9]+$/.test(key.raw) && Number(key.raw) < KEYS.length) ks.push(KEYS[Number(key.raw)]);
        else bad();
        vs.push(item(0));
      }
      let fields = { $: T.End };
      for (let k = n - 1; k >= 0; k -= 1) fields = { $: T.Field, key: ks[k], value: vs[k], tail: fields };
      return { $: T.Obj, fields };
    }
    return bad();
  };
  try {
    const v = item(0);
    return i === b.length ? v : { $: T.Null };
  } catch {
    return { $: T.Null };
  }
}

// a Bend Json tree as plain JavaScript (objects, arrays, strings, numbers,
// booleans, null), as JSON.parse would give its text: no recursion along a
// chain, only per level of nesting
export function plain(j) {
  switch (kind(j)) {
    case "Null": return null;
    case "Flag": return j.value;
    case "Num": return Number(j.raw);
    case "Str": return j.text;
    case "Arr": {
      const out = [];
      for (let c = j.items; is(c, "Item"); c = c.tail) out.push(plain(c.head));
      return out;
    }
    case "Obj": {
      const out = {};
      for (let c = j.fields; is(c, "Field"); c = c.tail) out[c.key] = plain(c.value);
      return out;
    }
    default: return null;
  }
}

// a JavaScript value as Bend's Json tree, exactly what core/json.bend's
// Json.parse gives for JSON.stringify(value) (Null where that text is
// undefined or malformed), without the text: the page used to stringify its
// session and the timeline's plan input and parse them again a character at a
// time on every render. A Bend String is a JS string, and Json.parse of what
// JSON.stringify writes gives back the very same code units, so strings pass
// through, except a lone surrogate (JSON.stringify writes it as \ud800):
// Bend's runtime refuses to make a Char of one ("not a Unicode scalar
// value"), so the old path threw out of the render, and one let through would
// reach the hub's decoder, which refuses it too. Here each becomes U+FFFD,
// keys and strings alike, as TextEncoder would send it. A number keeps the text JSON.stringify writes for it (String(n));
// keys go in JSON.stringify's order (Object.keys) and members it leaves out
// (undefined, functions, symbols) are left out here, or null in an array.
// Recursion only per level of nesting, as JSON.stringify. known (a WeakMap,
// optional) gives the tree of an object that came from plain() and is never
// changed afterwards, so a value handed back (the timeline's previous plan)
// costs nothing.
const NULL = () => ({ $: T.Null });
// s with each lone surrogate as U+FFFD (String.prototype.toWellFormed, which
// the phones' engines may lack; JavaScriptCore's /[\ud800-\udfff]/u misses
// some, so no regex)
export function whole(s) {
  if (s.isWellFormed) return s.isWellFormed() ? s : s.toWellFormed();
  let out = null, from = 0;
  for (let i = 0; i < s.length; i += 1) {
    const c = s.charCodeAt(i);
    if (c < 0xd800 || c > 0xdfff) continue;
    if (c < 0xdc00 && i + 1 < s.length) {
      const d = s.charCodeAt(i + 1);
      if (d >= 0xdc00 && d <= 0xdfff) { i += 1; continue; }
    }
    out = (out ?? "") + s.slice(from, i) + "\ufffd";
    from = i + 1;
  }
  return out === null ? s : out + s.slice(from);
}
function toTree(v, key, known) {
  if (known && v !== null && typeof v === "object") {
    const t = known.get(v);
    if (t) return t;
  }
  if (v !== null && (typeof v === "object" || typeof v === "bigint") && typeof v.toJSON === "function") v = v.toJSON(key);
  if (v !== null && typeof v === "object") {
    if (v instanceof Number) v = Number(v);
    else if (v instanceof String) v = String(v);
    else if (v instanceof Boolean) v = v.valueOf();
  }
  switch (typeof v) {
    case "string": return { $: T.Str, text: whole(v) };
    case "number": return Number.isFinite(v) ? { $: T.Num, raw: String(v) } : NULL();
    case "boolean": return { $: T.Flag, value: v };
    case "bigint": throw new TypeError("Do not know how to serialize a BigInt");
    case "object": {
      if (v === null) return NULL();
      if (Array.isArray(v)) {
        const n = v.length, vs = new Array(n);
        for (let k = 0; k < n; k += 1) vs[k] = toTree(v[k], String(k), known) || NULL();
        let items = { $: T.End };
        for (let k = n - 1; k >= 0; k -= 1) items = { $: T.Item, head: vs[k], tail: items };
        return { $: T.Arr, items };
      }
      const ks = Object.keys(v), fs = [];
      for (let k = 0; k < ks.length; k += 1) {
        const t = toTree(v[ks[k]], ks[k], known);
        if (t) fs.push(whole(ks[k]), t);
      }
      let fields = { $: T.End };
      for (let k = fs.length - 2; k >= 0; k -= 2) fields = { $: T.Field, key: fs[k], value: fs[k + 1], tail: fields };
      return { $: T.Obj, fields };
    }
    default: return undefined;
  }
}
export function json(value, known) {
  return toTree(value, "", known) || NULL();
}
