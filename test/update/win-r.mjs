// win-r.mjs PORT TOKEN_FILE CALL [params-json]: one call to a running hub over its own socket (update.check, update.status, app.update,
// hub.restart, ...), the reply printed as JSON. The R rows on Windows (test/update/win-r.ps1) drive an installed hub with it.
import {readFileSync} from 'node:fs';
import {client} from '../perf/lib.mjs';
const [port, tokf, m, p] = process.argv.slice(2);
const c = await client({host: '127.0.0.1', port: Number(port), token: readFileSync(tokf, 'utf8').trim()});
for (let i = 0; i < 50 && c.ws.readyState !== 1; i++) await new Promise(r => setTimeout(r, 100));
const r = await c.rpc(m, p ? JSON.parse(p) : {}, 30000);
console.log(JSON.stringify(r)); c.ws.close(); process.exit(0);
