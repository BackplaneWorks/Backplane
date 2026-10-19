import { readFileSync } from 'node:fs';
import { encode } from './ios_test_cbor';
import { cbor, plain } from '../../src/web/wire.js';
const [home] = process.argv.slice(2);
if (!home?.startsWith('/tmp/bp-phonebench-')) throw Error('isolated phone fixture required');
const events = readFileSync(home + '/events.jsonl','utf8').split('\n');
const threads = events.filter(l => l.includes('"$":"ThreadCreated"') && l.includes('zqrun')).map(l => JSON.parse(l).id);
if (threads.length !== 10) throw Error('need exact 10 fixture threads');
const src = readFileSync(new URL('../../src/core/cbor.bend', import.meta.url),'utf8');
const list = (name:string) => JSON.parse(src.split(`def Cbor.${name}() -> List<&2, String>:`)[1].split('\n\n')[0].replace(/\s+/g,' ').trim());
const keys = list('keys'), words = list('words');
let rid=1000000;
const runID=String(Date.now());
const waits = new Map<number, {ok:(v:any)=>void,bad:(e:any)=>void}>();
const ws = new WebSocket('ws://127.0.0.1:3897/ws?since=0&origin=');
ws.binaryType='arraybuffer';
ws.onmessage = event => {
 const o:any = plain(cbor(new Uint8Array(event.data as ArrayBuffer),keys,words));
 const p=waits.get(o?.id); if(p) { waits.delete(o.id); o.error ? p.bad(Error(JSON.stringify(o.error))) : p.ok(o); }
};
await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});
async function rpc(m:string,p:any) {
 const id=++rid; const a:number[]=[];encode({id,m,p},a);
 const answer=new Promise((ok,bad)=>{waits.set(id,{ok,bad});});ws.send(new Uint8Array(a));
 return await Promise.race([answer,new Promise((_,j)=>setTimeout(()=>j(Error('RPC timeout '+m)),15000))]);
}
for (const t of [...threads,'t1790292962-8017']) await rpc('thread.provider',{thread:t,provider:'claude'});
for (const [i,t] of threads.entries()) await rpc('turn.start',{thread:t,text:i%3===0?'zqsub phone responsive test':'phone responsive test',msg:'ios-stream-'+runID+'-'+i});
console.log('PASS exact10 fixture threads explicitly use stand-in Claude; Bub provider explicit too; verify sustained deltas separately');
ws.close();
process.exit(0);
