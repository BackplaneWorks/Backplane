// scripts/js-native.py's helpers against Base's definitions, as Bend reads
// them (String.cmp a code point at a time; Map.bit.go's bit of a key's code
// point; String.split's pieces between code points equal to sep; String.starts_with
// and String.contains by code points), on random strings with ASCII, BMP, astral and lone surrogates.
//
//   bun test/tools/js_native_test.ts
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const here = resolve(import.meta.dir, "../..");
const py = readFileSync(join(here, "scripts/js-native.py"), "utf8");
const src = /HELPERS = \(\n([\s\S]*?)\n\)/.exec(py)![1]
  .split("\n").map((l) => JSON.parse(l.trim().replace(/^"/, '"').replace(/\\n"$/, '\\n"'))).join("");
const { __bpOrd, __bpBit, __bpSplit, __bpPre, __bpHas } = new Function(src + "; return { __bpOrd, __bpBit, __bpSplit, __bpPre, __bpHas };")();

// Base: String.cmp walks code points (codePointAt > 0xFFFF takes two units)
const cps = (s: string) => { const out: number[] = []; for (let i = 0; i < s.length;) { const c = s.codePointAt(i)!; out.push(c); i += c > 0xffff ? 2 : 1; } return out; };
function refOrd(a: string, b: string) {
  const x = cps(a), y = cps(b);
  for (let i = 0; ; i++) {
    if (i === x.length) return i === y.length ? "EQ" : "LT";
    if (i === y.length) return "GT";
    if (x[i] !== y[i]) return x[i] < y[i] ? "LT" : "GT";
  }
}
// Base: Map.bit.go(key, ci, off): false past the end; off 0 true; else bit 31-(off-1)
function refBit(k: string, ci: number, off: number) {
  const x = cps(k);
  if (ci >= x.length) return false;
  if (off === 0) return true;
  return ((x[ci] >>> (31 - (off - 1))) & 1) !== 0;
}

// Base: String.split walks code points; a piece ends at each char equal to sep
function refSplit(s: string, sep: string) {
  const out: string[] = []; let cur = "";
  for (let i = 0; i < s.length;) { const c = s.codePointAt(i)!; const ch = c > 0xffff ? s.slice(i, i + 2) : s[i]; i += ch.length; if (ch === sep) { out.push(cur); cur = ""; } else cur += ch; }
  out.push(cur); return out;
}
// Base: String.starts_with and String.contains compare code points, and
// contains tries each code point of s as a start
function refPre(s: string, p: string) {
  const x = cps(s), y = cps(p);
  if (y.length > x.length) return false;
  for (let i = 0; i < y.length; i++) if (x[i] !== y[i]) return false;
  return true;
}
function refHas(s: string, p: string) {
  const x = cps(s), y = cps(p);
  for (let i = 0; i <= x.length; i++) {
    if (i === x.length) return y.length === 0;
    let ok = i + y.length <= x.length;
    for (let j = 0; ok && j < y.length; j++) if (x[i + j] !== y[j]) ok = false;
    if (ok) return true;
  }
  return false;
}
const unlist = (xs: any) => { const out: string[] = []; for (; xs.$ === "Con"; xs = xs.tail) out.push(xs.head); return out; };

const pool = ["a", "b", "z", "A", "0", "~", "é", "ж", "中", "￿", "", "😀", "😁", "𝄞", "\ud800", "\udc00", "\udbff"];
let seed = 7;
const rnd = (n: number) => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed % n; };
const str = () => { let s = ""; const n = rnd(6); for (let i = 0; i < n; i++) s += pool[rnd(pool.length)]; return s; };
let bad = 0;
for (let i = 0; i < 200000; i++) {
  const a = str(), b = rnd(4) === 0 ? a : rnd(3) === 0 ? a + str() : str();
  if (__bpOrd(a, b) !== refOrd(a, b)) { if (bad++ < 5) console.error("ord", JSON.stringify(a), JSON.stringify(b), __bpOrd(a, b), refOrd(a, b)); }
  const sep = pool[rnd(pool.length - 1)] || "\n";
  const got = JSON.stringify(unlist(__bpSplit(a + sep + b, sep))), want = JSON.stringify(refSplit(a + sep + b, sep));
  if (got !== want) { if (bad++ < 5) console.error("split", JSON.stringify(a + sep + b), JSON.stringify(sep), got, want); }
  // p: a piece of a (maybe cut through a surrogate pair), a's start, or another string
  const k = rnd(a.length + 1), m = rnd(a.length + 1);
  const p = rnd(3) === 0 ? str() : rnd(2) === 0 ? a.slice(Math.min(k, m), Math.max(k, m)) : a.slice(0, k);
  if (__bpPre(a, p) !== refPre(a, p)) { if (bad++ < 5) console.error("starts_with", JSON.stringify(a), JSON.stringify(p), __bpPre(a, p)); }
  if (__bpHas(a, p) !== refHas(a, p)) { if (bad++ < 5) console.error("contains", JSON.stringify(a), JSON.stringify(p), __bpHas(a, p)); }
  const ci = rnd(7), off = rnd(33);
  if (__bpBit(a, ci, off) !== refBit(a, ci, off)) { if (bad++ < 5) console.error("bit", JSON.stringify(a), ci, off); }
}
// Json.parse's memo (server only): a large text parses once and gives the
// same value again; small texts and texts past the last 8 parse afresh
const psrc = /PARSE_HELPER = \(\n([\s\S]*?)\n\)/.exec(py)![1]
  .split("\n").map((l) => JSON.parse(l.trim().replace(/\\n"$/, '\\n"'))).join("");
const { __bpParsed } = new Function(psrc + "; return { __bpParsed };")();
let calls = 0;
const parse = (t: string) => { calls++; return { $: "Some", value: JSON.parse(t) }; };
const big = JSON.stringify({ models: Array.from({ length: 2000 }, (_, i) => "model-" + i) });
const one = __bpParsed(big, parse), two = __bpParsed(big, parse);
if (one !== two || calls !== 1 || one.value.models.length !== 2000) { bad++; console.error("parse memo: a large text was parsed again", calls); }
__bpParsed("{}", parse); __bpParsed("{}", parse);
if (calls !== 3) { bad++; console.error("parse memo: a small text was remembered", calls); }
for (let i = 0; i < 9; i++) __bpParsed(big + " ".repeat(i + 1), parse);
__bpParsed(big, parse);
if (calls !== 13) { bad++; console.error("parse memo: kept more than 8 texts", calls); }
// Refs.last.go (src/core/refs.bend), as Bend reads it: a code point at a
// time, cur the reversed word so far, reset at a space, tab, CR or LF; at
// the end, cur reversed
const rsrc = /REFS_HELPER = \(\n([\s\S]*?)\n\)/.exec(py)![1]
  .split("\n").map((l) => JSON.parse(l.trim().replace(/\\n"$/, '\\n"'))).join("");
const { __bpRefsLast } = new Function(rsrc + "; return { __bpRefsLast };")();
function refLast(s: string, cur: string) {
  for (let i = 0; i < s.length;) {
    const c = s.codePointAt(i)!, ch = c > 0xffff ? s.slice(i, i + 2) : s[i]; i += ch.length;
    cur = c === 32 || c === 10 || c === 9 || c === 13 ? "" : ch + cur;
  }
  return [...cur].reverse().join("");
}
const words = ["", " ", "\n", "\t", "\r", "a", "/btw", "$skill", "@bot", "x😀y", "\ud83d", "\ude00", "é", "  ", "\r\n"];
for (let i = 0; i < 20000; i++) {
  const s = Array.from({ length: rnd(8) }, () => words[rnd(words.length)]).join("");
  const cur = rnd(3) === 0 ? "" : Array.from({ length: rnd(3) }, () => words[rnd(words.length)].replace(/[ \n\t\r]/g, "")).join("");
  const got = __bpRefsLast(s, cur), want = refLast(s, cur);
  if (got !== want) { if (bad++ < 5) console.error("refs.last", JSON.stringify(s), JSON.stringify(cur), JSON.stringify(got), JSON.stringify(want)); }
}
const long = "word ".repeat(40000) + "/tail";
const t0 = performance.now(); for (let i = 0; i < 100; i++) __bpRefsLast(long, "");
if (__bpRefsLast(long, "") !== "/tail" || performance.now() - t0 > 50) { bad++; console.error("refs.last: a 200 KB draft's word is not found from its end"); }
if (bad) { console.error(`js_native: ${bad} mismatches`); process.exit(1); }
console.log("js_native: ok");
