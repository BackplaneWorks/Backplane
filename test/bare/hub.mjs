// hub.mjs PORT TOKEN_FILE CMD...: a person's hub, from the harness side of the bare gate (runs on the Boat host, not in the clean box).
//   info PREFIX...            print the info keys starting with any PREFIX as one JSON object, then exit
//   wait KEY JSPRED SECS      wait until info KEY (parsed JSON, `v`) satisfies JSPRED (e.g. 'v && v.state=="done"'); print it; exit 1 on timeout
//   rpc METHOD PARAMS_JSON    one RPC; print its reply
import { readFileSync } from 'node:fs';
import { encode, decode } from '../wire.mjs';

const [port, tokenFile, cmd, ...rest] = process.argv.slice(2);
const token = readFileSync(tokenFile, 'utf8').trim();
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${token}`);
ws.binaryType = 'arraybuffer';
const info = new Map(); const replies = new Map(); let seen = 0;
const parse = v => { if (typeof v !== 'string') return v; try { return JSON.parse(v) } catch { return v } };
ws.onmessage = ({ data }) => {
  const x = decode(data); seen++;
  if (x.t === 'reply') replies.set(x.id, x);
  if (x.info && typeof x.info === 'object') for (const [k, v] of Object.entries(x.info)) info.set(k, parse(v));
};
await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws error')) });
const sleep = ms => new Promise(r => setTimeout(r, ms));
await sleep(400);
const out = o => { console.log(JSON.stringify(o)); ws.close(); process.exit(0) };
if (cmd === 'info') {
  const o = {}; for (const [k, v] of info) if (!rest.length || rest.some(p => k.startsWith(p))) o[k] = v; out(o);
} else if (cmd === 'wait') {
  const [key, pred, secs] = rest; const f = new Function('v', `return (${pred})`);
  const end = Date.now() + 1000 * Number(secs || 60);
  // the info key may change without a push to a view socket: ask again by reconnecting every 2 s
  while (Date.now() < end) {
    if (f(info.get(key))) out(info.get(key));
    await sleep(500);
  }
  console.log(JSON.stringify({ timeout: key, last: info.get(key) ?? null })); process.exit(1);
} else if (cmd === 'rpc') {
  const [m, p] = rest; const id = 2 ** 40 + 1;
  ws.send(encode({ id, m, p: JSON.parse(p || '{}') }));
  for (let i = 0; i < 2400 && !replies.has(id); i++) await sleep(25);
  out(replies.get(id) ?? { timeout: m });
} else { console.error('hub.mjs: info|wait|rpc'); process.exit(2) }
