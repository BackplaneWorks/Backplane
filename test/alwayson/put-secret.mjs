// Puts one secret into a hub's vault with secret.set, its value read from a file (never an argument): bun put-secret.mjs TOKEN_FILE URL NAME VALUE_FILE
import {readFileSync} from 'node:fs';
import {encode, decode} from '../wire.mjs';
const [tf, url, name, vf] = process.argv.slice(2);
const token = readFileSync(tf, 'utf8').trim(), value = readFileSync(vf, 'utf8').trim();
const ws = new WebSocket(`${url}/ws?token=${encodeURIComponent(token)}`); ws.binaryType = 'arraybuffer';
const quit = setTimeout(() => { console.error('no reply'); process.exit(3); }, 20000);
ws.onopen = () => ws.send(encode({id: 77, m: 'secret.set', p: {name, purpose: 'lane F e2e', thread: '', value}}));
ws.onmessage = ev => { let f; try { f = decode(ev.data); } catch { return; }
  for (const x of Array.isArray(f) ? f : [f]) if (x && x.t === 'reply' && x.id === 77) { clearTimeout(quit); console.log(x.ok === false ? 'refused: ' + x.error : 'set ' + name); process.exit(x.ok === false ? 1 : 0); } };
ws.onerror = () => { console.error('cannot reach', url); process.exit(4); };
