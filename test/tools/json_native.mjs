// wire.js's json(v) against Json.parse(JSON.stringify(v)) from the emitted
// test/json_native_test.bend: the same tree, field for field, on synthetic
// values (unicode, escapes, surrogates, big and odd numbers, toJSON, holes).
import assert from 'node:assert/strict';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { json, jsonText, learnJson, plain, kind, whole as wf } from '../../src/web/wire.js';

const F = (await import(pathToFileURL(path.resolve(process.argv[2])).href)).default;
learnJson(F.json_tags());

// a tree as nested arrays of its constructor names and fields, so a field
// either side lacks or has extra shows up
function shape(j) {
  const out = [];
  const go = (x) => {
    const k = kind(x), own = Object.keys(x).filter(f => f !== '$').sort();
    switch (k) {
      case 'Null': case 'End': return [k, own];
      case 'Flag': return [k, own, x.value];
      case 'Num': return [k, own, x.raw];
      case 'Str': return [k, own, x.text];
      case 'Arr': { const vs = []; for (let c = x.items; kind(c) === 'Item'; c = c.tail) vs.push(go(c.head)); return [k, own, vs]; }
      case 'Obj': { const vs = []; for (let c = x.fields; kind(c) === 'Field'; c = c.tail) vs.push([c.key, go(c.value)]); return [k, own, vs]; }
      default: return ['?', k];
    }
  };
  out.push(go(j));
  return out;
}
const tagsOf = (j) => { const s = new Set(); const go = x => { s.add(x.$); for (const f of ['items', 'fields', 'head', 'tail', 'value']) if (x[f] && typeof x[f] === 'object') go(x[f]); }; go(j); return s; };
const tagOk = (got, want) => { const w = tagsOf(want); return [...tagsOf(got)].every(t => w.has(t) || /End|Null/.test(t)); };

// JSON.stringify escapes a lone surrogate (\ud800), and Bend's JS runtime
// refuses to make a Char of one ("not a Unicode scalar value"): there the old
// path threw out of the render. json() makes each one U+FFFD, so it gives what
// Json.parse gives for the same value with them replaced.
const fix = (t) => { let o = ''; for (let i = 0; i < t.length; i += 1) { const c = t.charCodeAt(i), d = t.charCodeAt(i + 1);
  if (c >= 0xd800 && c < 0xdc00 && d >= 0xdc00 && d < 0xe000) { o += t[i] + t[i + 1]; i += 1; } else o += c >= 0xd800 && c < 0xe000 ? '\ufffd' : t[i]; } return o; };
const whole = (v) => typeof v === 'string' ? fix(v)
  : Array.isArray(v) ? v.map(whole)
  : v && typeof v === 'object' && typeof v.toJSON !== 'function' && !(v instanceof Number || v instanceof String || v instanceof Boolean)
    ? Object.fromEntries(Object.keys(v).map(k => [whole(k), whole(v[k])])) : v;
let n = 0, refused = 0;
function same(v, name) {
  const text = JSON.stringify(v);
  if (text !== undefined && /\\ud[89a-f][0-9a-f]{2}/i.test(text)) {
    assert.throws(() => F.parse(text), /not a Unicode scalar value/, name + ' (Bend refuses a lone surrogate)');
    const fixed = JSON.stringify(whole(v));
    assert.ok(!/\\ud[89a-f][0-9a-f]{2}/i.test(fixed), name + ' (replaced) ' + fixed);
    assert.deepEqual(shape(json(v)), shape(F.parse(fixed)), name + ' (U+FFFD in place)');
    refused += 1;
    return;
  }
  const want = text === undefined ? F.parse('') : F.parse(text);
  const got = json(v);
  assert.deepEqual(shape(got), shape(want), name);
  assert.ok(tagOk(got, want), name + ' (tags)');
  assert.equal(F.show(got), F.show(want), name + ' (Json.show)');
  if (text !== undefined) assert.deepEqual(plain(got), JSON.parse(text), name + ' (plain)');
  n += 1;
}

class Point { constructor() { this.x = 1; this.y = -2; } }
const big = [];
for (let i = 0; i < 2000; i += 1) big.push({ id: 't' + i, n: i * 1.5, ok: i % 2 === 0, k: [i, String(i), null] });
const values = {
  null: null, true: true, false: false, zero: 0, negzero: -0, int: 42, neg: -17, frac: 0.1 + 0.2,
  bigint_safe: Number.MAX_SAFE_INTEGER, huge: 1e21, tiny: 5e-324, neg_exp: -1.5e-7, maxnum: Number.MAX_VALUE,
  nan: NaN, inf: Infinity, ninf: -Infinity, empty_str: '', empty_arr: [], empty_obj: {},
  ascii: 'hello world', quotes: 'a "quoted" \\ back\\slash /slash',
  controls: '\b\f\n\r\t\u0000\u0001\u001f\u007f',
  unicode: 'Grüße · 日本語 · Ωmega · é (é)', emoji: '🙂👍🏽 👨‍👩‍👧 \u{1F600}\u{10FFFF}',
  lone_high: 'a\ud800b', lone_low: 'a\udc00b', reversed_pair: '\udc00\ud800', trailing_high: 'x\udbff',
  line_seps: '  ', bom: '﻿',
  keys: { 'ключ': 1, '🙂': 2, 'a"b': 3, 'c\\d': 4, '\n': 5, '\ud800': 6, '': 7 },
  int_keys_order: { b: 1, 2: 'two', a: 3, 1: 'one', '-1': 'm' },
  holes: [1, , 3], undef_arr: [undefined, () => 1, Symbol('s')], undef_obj: { a: undefined, b: () => 1, c: Symbol('s'), d: 1 },
  date: new Date(Date.UTC(2026, 9, 31, 12, 0, 0)), tojson: { toJSON: (k) => ({ key: k, v: [1, 2] }) },
  tojson_nested: { inner: { toJSON: (k) => 'key:' + k }, arr: [{ toJSON: (k) => 'idx:' + k }] },
  boxed: [new Number(3), new String('s'), new Boolean(false)], cls: new Point(), map: new Map([[1, 2]]),
  nested: { a: { b: { c: { d: [[[['deep']]]] } } } },
  session: { sel: 't1', drafts: { '@settings.page': '', t1: 'draft "x"\n' }, open: { settings: false }, info: { clock: '1793500000' }, list: big.slice(0, 5) },
  big,
};
for (const [name, v] of Object.entries(values)) same(v, name);
same(undefined, 'undefined');
same(() => 1, 'function');
same(Symbol('x'), 'symbol');
for (const v of Object.values(values)) same([v], 'wrapped');

// random values: strings of random code units (lone surrogates included)
let seed = 7;
const rnd = (m) => { seed = (seed + 0x6d2b79f5) >>> 0; let t = Math.imul(seed ^ (seed >>> 15), seed | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return Math.floor((((t ^ (t >>> 14)) >>> 0) / 4294967296) * m); };
const rstr = () => { let s = ''; const len = rnd(12); for (let i = 0; i < len; i += 1) { const c = rnd(9); s += c < 3 ? String.fromCharCode(rnd(128)) : c < 5 ? String.fromCharCode(rnd(0xd800)) : c < 8 ? String.fromCodePoint(0x10000 + rnd(0x100000)) : String.fromCharCode(rnd(4) ? 0xe000 + rnd(0x2000) : 0xd800 + rnd(0x800)); } return s; };
const rval = (d) => {
  switch (d > 3 ? rnd(4) : rnd(7)) {
    case 0: return rstr();
    case 1: return [rnd(1000) - 500, (rnd(1e6) - 5e5) / 7, rnd(2) ? 1e300 * rnd(10) : -rnd(100) * 1e-300][rnd(3)];
    case 2: return rnd(2) === 0;
    case 3: return null;
    case 4: { const a = []; const len = rnd(5); for (let i = 0; i < len; i += 1) a.push(rval(d + 1)); return a; }
    default: { const o = {}; const len = rnd(5); for (let i = 0; i < len; i += 1) o[rstr()] = rval(d + 1); return o; }
  }
};
for (let i = 0; i < 1500; i += 1) same(rval(0), 'random ' + i);

assert.throws(() => json(1n), TypeError);
// the fallback for engines without isWellFormed/toWellFormed gives the same
const { isWellFormed, toWellFormed } = String.prototype;
const texts = [];
for (let i = 0; i < 3000; i += 1) texts.push(rstr() + rstr());
const want = texts.map(t => toWellFormed.call(t));
delete String.prototype.isWellFormed; delete String.prototype.toWellFormed;
let got;
try { got = texts.map(wf); } finally { Object.assign(String.prototype, { isWellFormed, toWellFormed }); }
for (let i = 0; i < texts.length; i += 1) {
  assert.equal(got[i], want[i], 'fallback ' + JSON.stringify(texts[i]));
  assert.equal(fix(texts[i]), want[i], 'test fix ' + JSON.stringify(texts[i]));
}
// jsonText: JSON text straight to the tree Json.parse gives, raw numbers,
// field order and duplicate keys kept; null (use Bend) for anything else
let texts_n = 0, nulls = 0;
const sameText = (t, name) => {
  const got = jsonText(t);
  assert.ok(got, name + ' (read) ' + t.slice(0, 200));
  assert.deepEqual(shape(got), shape(F.parse(t)), name + ' ' + t.slice(0, 200));
  texts_n += 1;
};
const refuse = (t, name) => { assert.equal(jsonText(t), null, name + ' ' + JSON.stringify(t).slice(0, 200)); nulls += 1; };
for (const [name, v] of Object.entries(values)) {
  const t = JSON.stringify(v);
  if (t === undefined) continue;
  if (/\\ud[89a-f][0-9a-f]{2}/i.test(t)) refuse(t, 'text ' + name); else sameText(t, 'text ' + name);
}
for (const t of ['1.0', '-0', '1E5', '1e+5', '0.10', '-12.5e-3', '12345678901234567890', '1.7976931348623157e309',
  '{"b":1,"2":"two","a":3,"1":"one","-1":"m","4294967294":0}', '{"a":1,"a":2,"b":[1.0,2.50],"a":{"a":-0}}',
  ' \t\n\r[ 1 , {"x" : "y" } ,[ ] , { } ]\n', '"\\u00e9\\u0000\\/\\b\\f\\n\\r\\t\\"\\\\"', '"raw 🙂 é \u2028"',
  'null', 'true', 'false', '[]', '{}', '""', '{"__proto__":{"x":1},"constructor":2}', '[[[[[[[[[[1]]]]]]]]]]'])
  sameText(t, 'odd ' + t);
for (const t of ['', ' ', '[1 2]', '{"a" 1}', '{"a":1 "b":2}', '[1,]', '[,1]', '{,}', '[tru]', 'nul', 'True', 'undefined', '01', '1.', '.5', '+1', '-',
  '1e', '0x10', '"\\x"', '"\\u12"', '"\\u12g4"', '"\\ud800"', '"\\ud83d\\ude00"', '"a\u0001b"', '"open', '[1]x', '1 2', '{"a":1}}', '[1]]', "'a'",
  '{1:2}', '[NaN]', '[Infinity]', '"\ud800"', '"\udc00x"', '\u000b1', '[1\u00a0]'])
  refuse(t, 'refused');
// deep enough to pass any stack test.sh allows (512 MiB for JSC: 200000
// levels fit there and parsed, 2000000 too)
const deep = '['.repeat(20000000) + ']'.repeat(20000000);
assert.equal(jsonText(deep), null, 'nesting past the stack falls back');
// random texts: random values written with odd spacing and number forms,
// duplicate and integer-like keys, field order shuffled
const oddNum = () => [String(rnd(100)), '-0', rnd(9) + '.0', rnd(9) + 'E' + rnd(30), rnd(9) + 'e-' + rnd(30), '1' + '0'.repeat(rnd(30)), '-' + rnd(99) + '.' + rnd(99) + 'e+' + rnd(9)][rnd(7)];
const sp = () => [' ', '', '\n', '\t', ' \r\n '][rnd(5)];
const rtext = (d) => {
  switch (d > 3 ? rnd(4) : rnd(6)) {
    case 0: return JSON.stringify(rstr()).replace(/\\ud[89a-f][0-9a-f]{2}/gi, 'x');
    case 1: return oddNum();
    case 2: return ['true', 'false', 'null'][rnd(3)];
    case 3: return '"' + ['a', 'b', '1', '0', '10', '2'][rnd(6)] + '"';
    case 4: { const a = []; const len = rnd(5); for (let k = 0; k < len; k += 1) a.push(sp() + rtext(d + 1) + sp()); return '[' + a.join(',') + ']'; }
    default: { const a = []; const len = rnd(6); for (let k = 0; k < len; k += 1) a.push(sp() + JSON.stringify(['a', 'b', '1', '0', '10', '2', 'é', rstr()][rnd(8)]).replace(/\\ud[89a-f][0-9a-f]{2}/gi, 'x') + sp() + ':' + sp() + rtext(d + 1)); return '{' + a.join(',') + '}'; }
  }
};
for (let k = 0; k < 2000; k += 1) sameText(sp() + rtext(0) + sp(), 'random text ' + k);
// and broken copies of them: either refused or, when still strict JSON, equal
for (let k = 0; k < 2000; k += 1) {
  const t = rtext(0), at = rnd(t.length + 1);
  const b = t.slice(0, at) + [',', ':', ']', '}', '"', ' ', 'x', '\\', '1'][rnd(9)] + t.slice(at + rnd(2));
  let strict = true;
  try { JSON.parse(b); } catch { strict = false; }
  if (strict && b.isWellFormed() && !/\\ud[89a-f][0-9a-f]{2}/i.test(b)) sameText(b, 'broken but strict ' + k); else refuse(b, 'broken ' + k);
}
console.log(`ok jsonText: ${texts_n} texts equal to Json.parse, ${nulls} refused (left to Bend)`);
assert.ok(refused >= 50 && n >= 500, `coverage: ${n} compared, ${refused} refused`);
console.log(`ok json_native: ${n} values (${refused} with lone surrogates, which Bend refuses and json() makes U+FFFD), each equal to Json.parse(JSON.stringify(v))`);
