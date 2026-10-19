// Build current-type kept state from persisted events; no sockets/providers.
import { readFileSync, writeFileSync } from 'node:fs';
const [bridge, events, out] = process.argv.slice(2);
if (!bridge || !events || !out) throw Error('BRIDGE EVENTS OUT');
(0, eval)(readFileSync(bridge, 'utf8'));
const B = (globalThis as any).Backplane;
const utf = new TextEncoder();
function encode(v: any, a: number[]) {
  const head = (m: number, n: number) => {
    if (n < 24) a.push(m | n);
    else if (n < 256) a.push(m | 24, n);
    else if (n < 65536) a.push(m | 25, n >> 8, n & 255);
    else a.push(m | 26, (n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255);
  };
  if (v === null) { a.push(246); return; }
  if (typeof v === 'boolean') { a.push(v ? 245 : 244); return; }
  if (typeof v === 'number') {
    if (Number.isInteger(v) && v >= 0 && v < 2 ** 32) head(0, v);
    else { a.push(199); encode(String(v), a); }
    return;
  }
  if (typeof v === 'string') { const bytes = utf.encode(v); head(96, bytes.length); for (const b of bytes) a.push(b); return; }
  if (Array.isArray(v)) { head(128, v.length); for (const x of v) encode(x, a); return; }
  const entries = Object.entries(v); head(160, entries.length);
  for (const [k, x] of entries) { encode(k, a); encode(x, a); }
}
B.start('ios-profile', '{}');
const key = 'fixture:3897'; B.hubs([key]);
const lines = readFileSync(events, 'utf8').split('\n').filter(Boolean);
let count = 0;
for (let i = 0; i < lines.length; i += 512) {
  const items = lines.slice(i, i + 512).map(l => JSON.parse(l));
  const a: number[] = []; const more = i + 512 < lines.length;
  encode({t: i === 0 ? 'log' : 'changes', items, more, boot: more,
    info: {machines: JSON.stringify({self:{name:'Profile machine'}})}}, a);
  B.recv(key, Buffer.from(a).toString('base64'), true);
  count += items.length;
  if (i % 8192 === 0) console.log('folded', count);
}
const text = B.screen();
const s = JSON.parse(text).screen;
if (!s?.uber && !(s?.bots ?? []).some((b:any) => /Bub/.test(b.name))) throw Error('no Bub/Uberbot after replay');
writeFileSync(out, B.save());
writeFileSync(out + '.screen.json', text);
console.log('PASS', count, 'events; current state saved; hubs',s.hubs.length,'bots',s.bots.length,'uber',!!s.uber);
