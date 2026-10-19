// Plain CBOR test frames; product decoding remains src/web/wire.js.
const utf = new TextEncoder();
export function encode(v: any, a: number[]) {
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
