// A mobile-renderer view client (what GPUI and the phones are): open the recent view, pick the first thread, open it, then
// fetch view.update after each view.changed push (at most `maxPerSec` a second). Event driven; round trips are measured in onmessage.
import {client} from './lib.mjs';
const BASE=2**47+100;
export async function viewClient(h,{maxPerSec=5,onUpdate}={}){
 const c=await client(h,{view:true});
 const v={c,rtt:[],errors:0,updates:0,bytes:0,viewId:'',target:{kind:'recent',id:''},session:{sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}},rev:0,next:BASE,pending:false,dirty:false,last:0,stopped:false};
 for(let i=0;i<500&&!c.greeting;i++)await new Promise(r=>setTimeout(r,10)); // connect only: a greeting is pushed at accept
 if(!c.greeting)throw Error('no view greeting');
 const ask=async(m,extra={})=>{const id=++v.next;const p={...extra,version:1,hub:h.url,generation:c.greeting.generation,epoch:c.greeting.epoch,requestRevision:++v.rev,target:v.target,viewId:v.viewId,session:v.session,cid:'soak'};
  const t0=performance.now();
  const r=await new Promise(res=>{const to=setTimeout(()=>{c.waiters.delete(id);res(undefined)},60000);c.waiters.set(id,x=>{clearTimeout(to);res(x)});try{c.ws.send(c.enc({id,m,p}))}catch{clearTimeout(to);res(undefined)}});
  if(!r)return {ms:performance.now()-t0,ok:false,timeout:true};
  const x=r.o;if(x.ok&&x.result?.session)Object.assign(v.session,x.result.session);if(x.ok&&x.result?.viewId)v.viewId=x.result.viewId;
  return {ms:r.t-t0,ok:!!x.ok,x,n:0}};
 v.ask=ask;
 const o1=await ask('view.open');if(!o1.ok)throw Error('view.open recent: '+JSON.stringify(o1.x?.error??o1));
 const tid=o1.x.result?.shown?.projects?.[0]?.threads?.[0]?.id;
 if(tid){const sel=await ask('view.action',{action:'select',value:tid});const nt=sel.x?.result?.nextTarget;if(nt){v.target=nt;v.viewId='';const o2=await ask('view.open');if(!o2.ok)throw Error('view.open thread failed')}}
 v.openMs=o1.ms;
 const fetchOne=async()=>{if(v.pending||v.stopped)return;const wait=1000/maxPerSec-(performance.now()-v.last);if(wait>0){setTimeout(fetchOne,wait);return}
  v.pending=true;v.dirty=false;v.last=performance.now();const r=await ask('view.update');v.pending=false;v.updates++;if(r.ok)v.rtt.push(r.ms);else{v.errors++;v.lastError=r.x?.error??'timeout'}onUpdate?.(r);if(v.dirty)fetchOne()};
 c.onView=o=>{if(o.t==='view.changed'){v.dirty=true;fetchOne()}};
 return v;
}
