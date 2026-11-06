// bun window_e2e.mjs HOME PORT : view.open each reference thread on backplane-hub and compare the reply's `window`
// with the JS hub's (fixtures/windows.json; the older cursor carries the hub's epoch, so that is normalised).
import {readFileSync} from 'node:fs';
import {encode,decode} from '../wire.mjs';
const [home,port]=process.argv.slice(2);const token=readFileSync(home+'/token','utf8').trim();
const refs=JSON.parse(readFileSync(new URL('./fixtures/windows.json',import.meta.url),'utf8'));
const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${token}`);ws.binaryType='arraybuffer';
let greet;const waiting=new Map();
ws.onmessage=({data})=>{const x=decode(data);if(x.t==='view')greet=x;if(x.t==='reply'&&waiting.has(x.id)){waiting.get(x.id)(x);waiting.delete(x.id)}};
await new Promise(r=>ws.onopen=r);for(let i=0;i<300&&!greet;i++)await new Promise(r=>setTimeout(r,10));
let n=2**47+10,rev=0;const session={sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}};
const open=async(thread)=>{const id=++n;const target={kind:'thread',id:thread};const p={version:1,hub:'http://x',generation:greet.generation,epoch:greet.epoch,requestRevision:++rev,target,viewId:'',session:{...session,sel:thread},cid:'e2e'};
 const pr=new Promise(r=>waiting.set(id,r));const t=performance.now();ws.send(encode({id,m:'view.open',p}));const x=await pr;return {x,ms:performance.now()-t}};
const norm=(w,e)=>JSON.parse(JSON.stringify(w).split(e).join('E')); // the hub's epoch appears in every cursor string
let bad=0;const times=[];
for(const r of refs){const {x,ms}=await open(r.thread);times.push(ms);const got=x.result?.window;
 if(!got){console.log(r.thread,'NO WINDOW',JSON.stringify(x).slice(0,300));bad++;continue}
 const a=norm(got,greet.epoch),b=norm(r.window,r.epoch);const keys=[...new Set([...Object.keys(a),...Object.keys(b)])].filter(k=>JSON.stringify(a[k])!==JSON.stringify(b[k]));
 console.log(r.thread,keys.length?'DIFFERS in '+keys.join(','):'window equal',ms.toFixed(1)+' ms');bad+=keys.length?1:0}
console.log(JSON.stringify({windowsDiffering:bad,of:refs.length,open_ms:times.map(t=>+t.toFixed(1))}));ws.close();process.exit(0);
