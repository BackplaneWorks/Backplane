// scripts/js-native.py's helpers against Base's definitions, as Bend reads
// them (String.cmp a code point at a time; Map.bit.go's bit of a key's code
// point; String.split's pieces between code points equal to sep), on random strings with ASCII, BMP, astral and lone surrogates.
//
//   bun test/tools/js_native_test.ts
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const here = resolve(import.meta.dir, "../..");
const py = readFileSync(join(here, "scripts/js-native.py"), "utf8");
const src = /HELPERS = \(\n([\s\S]*?)\n\)/.exec(py)![1]
  .split("\n").map((l) => JSON.parse(l.trim().replace(/^"/, '"').replace(/\\n"$/, '\\n"'))).join("");
const { __bpOrd, __bpBit, __bpSplit } = new Function(src + "; return { __bpOrd, __bpBit, __bpSplit };")();

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
  const ci = rnd(7), off = rnd(33);
  if (__bpBit(a, ci, off) !== refBit(a, ci, off)) { if (bad++ < 5) console.error("bit", JSON.stringify(a), ci, off); }
}
if (bad) { console.error(`js_native: ${bad} mismatches`); process.exit(1); }
console.log("js_native: ok");
