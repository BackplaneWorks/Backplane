// node rpc.mjs PORT TOKEN METHOD 'JSON params' : one call to a hub over its
// /ws (CBOR frames, src/core/cbor.bend), its reply printed as JSON. Known map
// keys travel as their index in Cbor.keys(); the rest as strings.
const KEYS = ['$', 't', 'id', 'm', 'p', 'ok', 'error', 'items', 'thread'];
const enc = (v) => {
  const out = [];
  const head = (mt, n) => {
    if (n < 24) out.push(mt << 5 | n);
    else if (n < 256) out.push(mt << 5 | 24, n);
    else if (n < 65536) out.push(mt << 5 | 25, n >> 8, n & 255);
    else out.push(mt << 5 | 26, (n >>> 24) & 255, (n >> 16) & 255, (n >> 8) & 255, n & 255);
  };
  const go = (v) => {
    if (typeof v === 'number') head(0, v);
    else if (typeof v === 'boolean') out.push(v ? 0xf5 : 0xf4);
    else if (typeof v === 'string') { const b = Buffer.from(v, 'utf8'); head(3, b.length); out.push(...b); }
    else if (Array.isArray(v)) { head(4, v.length); v.forEach(go); }
    else { const ks = Object.keys(v); head(5, ks.length); for (const k of ks) { const ix = KEYS.indexOf(k); go(ix >= 0 ? ix : k); go(v[k]); } }
  };
  go(v);
  return new Uint8Array(out);
};
const dec = (b) => {
  let i = 0;
  const len = (ai) => ai < 24 ? ai : ai === 24 ? b[i++] : ai === 25 ? (i += 2, b[i - 2] << 8 | b[i - 1])
    : ai === 26 ? (i += 4, ((b[i - 4] << 24) >>> 0) + (b[i - 3] << 16) + (b[i - 2] << 8) + b[i - 1])
    : (i += 8, Number(Buffer.from(b.slice(i - 8, i)).readBigUInt64BE()));
  const go = () => {
    const x = b[i++], mt = x >> 5, ai = x & 31;
    if (mt === 7) {
      if (ai === 20) return false; if (ai === 21) return true; if (ai === 22) return null;
      if (ai === 27) { i += 8; return Buffer.from(b.slice(i - 8, i)).readDoubleBE(); }
      if (ai === 26) { i += 4; return Buffer.from(b.slice(i - 4, i)).readFloatBE(); }
      return null;
    }
    const n = len(ai);
    if (mt === 0) return n; if (mt === 1) return -1 - n;
    if (mt === 2 || mt === 3) { const s = Buffer.from(b.slice(i, i + n)); i += n; return mt === 3 ? s.toString('utf8') : s; }
    if (mt === 4) { const a = []; for (let k = 0; k < n; k++) a.push(go()); return a; }
    if (mt === 5) { const o = {}; for (let k = 0; k < n; k++) { const key = go(); o[typeof key === 'number' && KEYS[key] ? KEYS[key] : key] = go(); } return o; }
    if (mt === 6) return go();
  };
  return go();
};
// connect(port, token): one /ws for many calls; call(method, params) is the
// reply's JSON, and throws on {ok: false}
export const connect = (port, token) => new Promise((ready, fail) => {
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);
  ws.binaryType = 'arraybuffer';
  const waiting = new Map();
  let n = 0, up = false;
  const call = (m, p = {}) => new Promise((ok, no) => {
    const id = ++n;
    waiting.set(id, (o) => o.ok === false ? no(new Error(`${m}: ${JSON.stringify(o.error ?? o)}`)) : ok(o));
    ws.send(enc({ id, m, p }));
  });
  ws.onmessage = (e) => {
    const o = dec(new Uint8Array(e.data));
    if (!up) { up = true; ready({ call, close: () => ws.close() }); }
    const w = o && waiting.get(o.id);
    if (w) { waiting.delete(o.id); w(o); }
  };
  ws.onerror = (e) => fail(new Error(`ws error ${e.message ?? e}`));
});

if (import.meta.url === `file://${process.argv[1]}`) {
  const [port, token, method, params = '{}'] = process.argv.slice(2);
  setTimeout(() => { console.error('no reply'); process.exit(1); }, 20000);
  connect(port, token)
    .then((h) => h.call(method, JSON.parse(params)))
    .then((o) => { console.log(JSON.stringify(o)); process.exit(0); },
          (e) => { console.error(e.message); process.exit(1); });
}
