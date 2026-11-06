// hub_rpc.mjs PORT TOKEN_FILE JSON : plain hub RPCs the way a phone sends them, over the same websocket the clients use.
// JSON is a list of {m, p, wait?}; prints one line per reply ({"m","ok","error"}) and then every info frame seen whose text contains "teach.draft" (the teach draft).
import { readFileSync } from 'node:fs';
import { encode, decode } from '../wire.mjs';
const [port, tokenFile, steps] = process.argv.slice(2);
const token = readFileSync(tokenFile, 'utf8').trim();
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${token}`); ws.binaryType = 'arraybuffer';
const replies = new Map(), drafts = []; let id = 2 ** 40;
ws.onmessage = ({ data }) => { const x = decode(data); if (x.t === 'reply') replies.set(x.id, x); const t = JSON.stringify(x); if (t.includes('teach.draft')) drafts.push(t.slice(0, 4000)); };
await new Promise((r, j) => { ws.onopen = r; ws.onerror = () => j(new Error('ws error')); });
const sleep = ms => new Promise(r => setTimeout(r, ms));
await sleep(500);
for (const s of JSON.parse(steps)) {
  const my = ++id; ws.send(encode({ id: my, m: s.m, p: s.p }));
  for (let i = 0; i < 400 && !replies.has(my); i++) await sleep(25);
  const r = replies.get(my); console.log(JSON.stringify({ m: s.m, ok: r?.ok ?? null, error: r?.error ?? r?.err ?? '' }));
  if (s.wait) await sleep(s.wait);
}
await sleep(800);
for (const d of drafts.slice(-1)) console.log('DRAFT ' + d);
ws.close(); process.exit(0);
