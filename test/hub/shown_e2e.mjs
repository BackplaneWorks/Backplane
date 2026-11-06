// bun window_e2e.mjs HOME PORT : view.open each reference thread on backplane-hub and compare the reply's `window`
// with the JS hub's (fixtures/shown.json; the older cursor carries the hub's epoch, so that is normalised).
import {readFileSync} from 'node:fs';
import {encode,decode} from '../wire.mjs';
const [home,port]=process.argv.slice(2);const token=readFileSync(home+'/token','utf8').trim();
const fx=JSON.parse(readFileSync(new URL('./fixtures/shown.json',import.meta.url),'utf8'));
const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${token}`);ws.binaryType='arraybuffer';
let greet;const waiting=new Map();
ws.onmessage=({data})=>{const x=decode(data);if(x.t==='view')greet=x;if(x.t==='reply'&&waiting.has(x.id)){waiting.get(x.id)(x);waiting.delete(x.id)}};
await new Promise(r=>ws.onopen=r);for(let i=0;i<300&&!greet;i++)await new Promise(r=>setTimeout(r,10));
let n=2**47+10,rev=0;const session={sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}};
const open=async(thread,m='view.open',extra={})=>{if(m==='view.open'){}const id=++n;const target={kind:'thread',id:thread};const p={version:1,hub:'http://127.0.0.1:3818',generation:greet.generation,epoch:greet.epoch,requestRevision:++rev,target,viewId:extra.viewId||'',...extra.p,session,cid:'perf-c'};
 const pr=new Promise(r=>waiting.set(id,r));const t=performance.now();ws.send(encode({id,m,p}));const x=await pr;if(x.result?.session)Object.assign(session,x.result.session);return {x,ms:performance.now()-t}};

const at=(o,p)=>p.split('/').reduce((a,k)=>a==null?a:a[k],o);
const timey=p=>/\/(stamp|when|changes|seq)$/.test(p)||/\/picker\/models/.test(p); // changes counts the reference hub's housekeeping; the model list is the installed CLIs' (agents stream)
const diff=(a,b,path='',out=[])=>{if(JSON.stringify(a)===JSON.stringify(b))return out;
 if(a&&b&&typeof a==='object'&&typeof b==='object'&&!Array.isArray(a)===!Array.isArray(b)){for(const k of new Set([...Object.keys(a),...Object.keys(b)]))diff(a[k],b[k],path+'/'+k,out)}else if(!timey(path))out.push(path);return out};
const norm=(w,e)=>w===undefined?null:JSON.parse(JSON.stringify(w).split(e).join('E'));
let bad=0;
for(const r of fx.threads){const {x,ms}=await open(r.thread);const res=x.result||{};if(!x.result)console.log('ERR',JSON.stringify(x).slice(0,300));
 const d=[...diff(norm(res.shown,greet.epoch),norm(r.shown,r.epoch),'shown'),...diff(norm(res.overlay,greet.epoch),norm(r.overlay,r.epoch),'overlay')];
 console.log(r.thread,d.length?'DIFFERS '+d.slice(0,6).join(' '):'shown+overlay equal',ms.toFixed(1)+' ms');if(d.length&&process.env.VERBOSE){const R={shown:norm(res.shown,greet.epoch),overlay:norm(res.overlay,greet.epoch)},F={shown:norm(r.shown,r.epoch),overlay:norm(r.overlay,r.epoch)};for(const q of d.slice(0,4))console.log('  ',q,'\n   rust',JSON.stringify(at(R,q)??null).slice(0,260),'\n   js  ',JSON.stringify(at(F,q)??null).slice(0,260))}bad+=d.length?1:0}
for(const pg of fx.pages){const o=await open(pg.thread);const cursor=JSON.parse(JSON.stringify(pg.cursor).split(pg.epoch).join(greet.epoch));
 const {x,ms}=await open(pg.thread,'view.page',{viewId:o.x.result.viewId,p:{cursor}});const res=x.result||{};
 if(!res.window){console.log('page',pg.thread,'NO RESULT',JSON.stringify(x).slice(0,300));bad++;continue}
 const d=[...diff(norm(res.window,greet.epoch),norm(pg.window,pg.epoch),'window'),...diff(norm(res.shown,greet.epoch),norm(pg.shown,pg.epoch),'shown')];
 console.log('page',pg.thread,d.length?'DIFFERS '+d.slice(0,6).join(' '):'page equal',ms.toFixed(1)+' ms');if(d.length&&process.env.VERBOSE){const R={window:norm(res.window,greet.epoch),shown:norm(res.shown,greet.epoch)},F={window:norm(pg.window,pg.epoch),shown:norm(pg.shown,pg.epoch)};for(const q of d.slice(0,3))console.log('  ',q,'\n   rust',JSON.stringify(at(R,q)??null).slice(0,400),'\n   js  ',JSON.stringify(at(F,q)??null).slice(0,400))}bad+=d.length?1:0}
console.log(JSON.stringify({differing:bad}));ws.close();process.exit(0);
