// bun bench.ts URL N CONC : sequential then concurrent GETs, new connection each (Connection: close),
// prints median/p95/p99 ms and requests per second.
const [url, nArg, concArg = '1'] = process.argv.slice(2);
const n = Number(nArg), conc = Number(concArg);
const lat: number[] = [];
async function one() { const t = performance.now(); const r = await fetch(url, {keepalive: false, headers: {Connection: 'close'}}); await r.arrayBuffer(); lat.push(performance.now() - t); }
for (let i = 0; i < Math.min(50, n); i++) await one();   // warm
lat.length = 0;
const t0 = performance.now();
let left = n;
await Promise.all(Array.from({length: conc}, async () => { while (left-- > 0) await one(); }));
const wall = performance.now() - t0;
lat.sort((a, b) => a - b);
const q = (p: number) => lat[Math.min(lat.length - 1, Math.floor(p * lat.length))];
console.log(JSON.stringify({url, n, conc, median_ms: +q(0.5).toFixed(3), p95_ms: +q(0.95).toFixed(3), p99_ms: +q(0.99).toFixed(3), rps: +(n / (wall / 1000)).toFixed(1)}));
