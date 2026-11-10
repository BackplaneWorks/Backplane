// One RPC to a hub, its reply as JSON on stdout: bun scripts/hub-rpc.mjs TOKEN_FILE URL METHOD [PARAMS_JSON]
// URL is ws(s)://host:port (the hub's own address; a tailnet hub is wss://name.ts.net). Used by the Boat hub cutover (docs/BOAT-HUB.md).
import {readFileSync} from 'node:fs';
import {encode, decode} from '../test/wire.mjs';
const [tf, url, m, pj] = process.argv.slice(2);
if (!tf || !url || !m) { console.error('usage: hub-rpc.mjs TOKEN_FILE URL METHOD [PARAMS_JSON]'); process.exit(2); }
const token = readFileSync(tf, 'utf8').trim();
const ws = new WebSocket(`${url.replace(/\/$/, '')}/ws?token=${encodeURIComponent(token)}`); ws.binaryType = 'arraybuffer';
const id = 9000 + Math.floor(Math.random() * 1000);
const quit = setTimeout(() => { console.error('no reply in 30 s'); process.exit(3); }, 30000);
ws.onopen = () => ws.send(encode({id, m, p: pj ? JSON.parse(pj) : {}}));
ws.onmessage = ev => { let f; try { f = decode(ev.data); } catch { return; }
  for (const x of Array.isArray(f) ? f : [f]) if (x && x.t === 'reply' && x.id === id) { clearTimeout(quit); console.log(JSON.stringify(x)); process.exit(x.ok === false ? 1 : 0); } };
ws.onerror = e => { console.error('cannot reach', url, e.message || ''); process.exit(4); };
