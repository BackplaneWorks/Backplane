// bun test/e2e/rpc.mjs HOME METHOD ['JSON params'] : one call to a TEST hub (its home names the port and token: hub.lock, token), the reply
// printed as JSON. For poking a journey's hub by hand (`rig-hub.mjs` keeps one up): e.g.
//   bun test/e2e/rpc.mjs out/e2e-run/gpui-lived/home setting.set '{"key":"settle.idle.minutes","value":"1"}'
// Refuses a live home (lib/hubfx.mjs LIVE_HOMES): never the person's hub.
import {readFileSync,realpathSync} from 'node:fs';
import {client} from '../perf/lib.mjs';
import {LIVE_HOMES} from './lib/hubfx.mjs';
const [home,m,p='{}']=process.argv.slice(2);
if(!home||!m){console.error('usage: rpc.mjs HOME METHOD [JSON params]');process.exit(2)}
const r=realpathSync(home);if(LIVE_HOMES.some(l=>r===process.env.HOME+'/'+l||r.startsWith(process.env.HOME+'/'+l+'/'))){console.error('refused: '+r+' is a live home');process.exit(2)}
const port=Number(readFileSync(home+'/hub.lock','utf8').split(/\s+/)[1]),token=readFileSync(home+'/token','utf8').trim();
const c=await client({host:'127.0.0.1',port,token});const x=await c.rpc(m,JSON.parse(p),20000);
console.log(JSON.stringify(x?.o??{error:'no reply'}));c.ws.close();process.exit(x?.o?.ok===false||!x?1:0);
