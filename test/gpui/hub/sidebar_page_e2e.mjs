// bun sidebar_page_e2e.mjs HOME PORT : 300 threads, nativeSidebar offered: the first screen is the bounded form and the pager's
// next control moves the page (offset 0 -> limit -> 2*limit), each action in the same view.
import {readFileSync} from 'node:fs';
import {encode,decode} from '../wire.mjs';
const [home,port]=process.argv.slice(2);const token=readFileSync(home+'/token','utf8').trim();
const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${token}`);ws.binaryType='arraybuffer';
let greet;const waiting=new Map();
ws.onmessage=({data})=>{const x=decode(data);if(x.t==='view')greet=x;if(x.t==='reply'&&waiting.has(x.id)){waiting.get(x.id)(x);waiting.delete(x.id)}};
await new Promise(r=>ws.onopen=r);for(let i=0;i<300&&!greet;i++)await new Promise(r=>setTimeout(r,10));
let n=2**47+10,rev=0;const session={sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}};
let viewId='';
const ask=async(m,extra={})=>{const id=++n;const p={version:1,hub:'http://127.0.0.1:3815',generation:greet.generation,epoch:greet.epoch,requestRevision:++rev,target:{kind:'recent',id:''},viewId,session,cid:'e2e',nativeSidebar:1,...extra};
 const pr=new Promise(r=>waiting.set(id,r));ws.send(encode({id,m,p}));const x=await pr;const res=x.result;if(res?.session)Object.assign(session,res.session);if(res?.viewId)viewId=res.viewId;return x};
const find=(o,f,out=[])=>{if(o&&typeof o==='object'){if(f(o))out.push(o);for(const v of Object.values(o))find(v,f,out)}return out};
const pagers=r=>find(r.result.shown,o=>o&&typeof o==='object'&&'offset' in o&&'limit' in o&&'total' in o);
const pager=r=>pagers(r).sort((a,b)=>b.total-a.total)[0]; // the threads list's: the biggest
const nexts=r=>find(r.result.shown,o=>o.action==='native-sidebar-page');
let bad=0;const log=(ok,m)=>{console.log((ok?'ok   ':'FAIL ')+m);if(!ok)bad++};
const r0=await ask('view.open');
log(r0.ok&&r0.result?.remoteReady===true,'open answers a picture: '+JSON.stringify(r0.error||'').slice(0,80));
console.log('viewRevision at open: '+r0.result.viewRevision);
console.log('pagers: '+JSON.stringify(pagers(r0).map(p=>[p.total,p.offset,p.limit,!!p.next])));
const p0=pager(r0);log(!!p0&&p0.offset===0&&p0.total>=300,'first page: total='+p0?.total+' offset='+p0?.offset+' limit='+p0?.limit);
let offsets=[p0?.offset];let r=r0;
for(let i=0;i<3;i++){
  const pg=pager(r);const c=pg&&pg.next&&pg.next.action==='native-sidebar-page'?pg.next:null;
  if(!c){log(false,'no next control at step '+i);break}
  r=await ask('view.action',{action:'native-sidebar-page',value:c.value});
  if(!r.ok||!r.result?.shown){log(false,'page action refused: '+JSON.stringify(r).slice(0,200));break}
  const p=pager(r);offsets.push(p?.offset);
  console.log('  action reply: remoteReady='+r.result.remoteReady+' cmds='+JSON.stringify(r.result.cmds).slice(0,120)+' session.error='+JSON.stringify(r.result.session?.error)+' note='+JSON.stringify(r.result.session?.note)+' offset='+p?.offset+' tokenSame='+(p?.next?.value===c.value)+' cmdsLen='+(r.result.cmds||[]).length+' viewRevision='+r.result.viewRevision);
}
log(offsets.length===4&&offsets.every((o,i)=>i===0||o>offsets[i-1]),'the pager advances: offsets '+offsets.join(','));
console.log(JSON.stringify({failures:bad}));ws.close();await new Promise(r=>process.stdout.write('',r));await new Promise(r=>setTimeout(r,200));process.exit(0);
