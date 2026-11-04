// Plain-TS mirror of native/bend/history.bend (the pure half of Ts.page).
import * as ts from '../../../perf/bend-vs-ts/equivalents';
const [, repsArg = '30', innerArg = '1'] = process.argv.slice(2);
const reps = Number(repsArg), inner = Number(innerArg) || 1, M = 4294967291;
const h1 = (x: number) => (x * 48271) % 2147483647;
const h = (i: number) => h1(h1(i + 12345));
const mix = (a: number, v: number) => (a * 31337 + v) % M;
const text = (len: number) => { let s = ''; for (let p = 0; p < len; p++) s += p % 97 === 96 ? 'é' : 'x'; return s; };
const queries: { ord: number; text: string }[][] = [];
let t0 = performance.now();
for (let q = 0; q < 16; q++) {
  const rows = [];
  for (let j = 0; j < 49; j++) { const ord = 100000 + q * 1000 + (49 - j); rows.push({ ord, text: text(40 + (h(ord) % 1900)) }); }
  queries.push(rows);
}
console.log('queries ' + queries.length); console.log('gen_ms ' + Math.round(performance.now() - t0));
function once(): number {
  let acc = 0;
  queries.forEach((rows, q) => {
    let sum = 0;
    const built = rows.map(r => ({ ord: r.ord, blocks: ts.blocks(r.text) }));
    for (const r of built) {
      let a = mix(r.ord, r.blocks.length);
      for (const b of r.blocks) a = mix(a, [...b].length);
      sum = (sum + a) % M;
    }
    const ids = built.map(r => r.ord).sort((x, y) => y - x).slice(0, 32);
    let a = 0; for (const id of ids) a = mix(a, id);
    acc = mix(acc, (a + sum) % M);
  });
  return acc;
}
for (let r = 0; r < reps; r++) {
  t0 = performance.now(); let d = 0; for (let k = 0; k < inner; k++) d = once(); const ms = (performance.now() - t0) / inner;
  console.log('digest ' + d); console.log('ms ' + ms.toFixed(4));
}
