// hub_rpc.mjs PORT TOKEN_FILE JSON : send CBOR websocket RPCs to the hub and print replies + editor.kicad.* info.
import { readFileSync } from 'node:fs';
import { encode, decode } from '../../wire.mjs';

const [port, tokenFile, stepsJson] = process.argv.slice(2);
const token = readFileSync(tokenFile, 'utf8').trim();
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${token}`);
ws.binaryType = 'arraybuffer';

const replies = new Map();
const infos = new Map();
let id = 2 ** 40;

ws.onmessage = ({ data }) => {
  const x = decode(data);
  if (x.t === 'reply') replies.set(x.id, x);
  if (x.info && typeof x.info === 'object') {
    for (const [k, v] of Object.entries(x.info)) {
      if (
        k.startsWith('editor.kicad.') ||
        k.startsWith('kicad.sel.') ||
        k.startsWith('kicad.reload.') ||
        k.startsWith('kicad.comment.') ||
        k.startsWith('kicad.checks.')
      ) {
        infos.set(k, v);
      }
    }
  }
};

await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws error')); });
const sleep = ms => new Promise(r => setTimeout(r, ms));
await sleep(250);

for (const s of JSON.parse(stepsJson || '[]')) {
  const my = ++id;
  const method = s.m ?? s.method;
  const params = s.p ?? s.params ?? {};
  ws.send(encode({ id: my, m: method, p: params }));
  const maxIter = Math.ceil((s.timeout_ms ?? 15000) / 25);
  for (let i = 0; i < maxIter && !replies.has(my); i++) await sleep(25);
  const r = replies.get(my);
  console.log(JSON.stringify({ m: method, ok: r?.ok ?? null, result: r?.result ?? r ?? null, reply: r ?? null, error: r?.error ?? r?.err ?? '' }));
  if (s.wait) await sleep(s.wait);
}

await sleep(250);
for (const [k, v] of infos.entries()) {
  let parsed = null;
  try { parsed = v ? JSON.parse(v) : null; } catch {}
  console.log('INFO ' + JSON.stringify({ key: k, raw: v, value: parsed }));
}
ws.close();
process.exit(0);
