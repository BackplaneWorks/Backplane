// hub_info.mjs PORT TOKEN_FILE KEY...: the hub's info keys as a plain client sees them on connect (the first frame's `info`, then any
// `info` frame within a second), printed as one JSON object. hub.mjs's `info` reads a view socket, which carries no info on connect.
import { readFileSync } from 'node:fs';
import { decode } from '../wire.mjs';
const [port, tokenFile, ...keys] = process.argv.slice(2);
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?enc=cbor&token=${readFileSync(tokenFile, 'utf8').trim()}`);
ws.binaryType = 'arraybuffer';
const got = {};
const take = (m) => { if (m && typeof m === 'object') for (const [k, v] of Object.entries(m)) if (!keys.length || keys.includes(k)) got[k] = v; };
ws.onmessage = ({ data }) => { const x = decode(data); take(x.info); };
ws.onerror = () => { console.log(JSON.stringify({ error: 'ws error' })); process.exit(1); };
setTimeout(() => { console.log(JSON.stringify(got)); process.exit(0); }, 1500);
